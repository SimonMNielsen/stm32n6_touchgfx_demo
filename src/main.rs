//! TouchGFX on Rust — STM32N6570-DK.
//!
//! The C++ TouchGFX application vendored in `touchgfx_project/` (800×480
//! RGB565, no RTOS) runs on this board with **all** hardware access going
//! through the Rust N6 BSP + embassy — no ST HAL anywhere:
//!
//!   - LTDC / panel / backlight  → `bsp_stm32n6570::display`, full-screen
//!   - framebuffers (PSRAM) + double-buffer swap → cxx bridge → embassy PAC
//!   - vsync pacing → embassy ticker task on the HIGH interrupt executor
//!   - touch → GT911, sampled from thread mode over the bridge
//!   - ChromART (DMA2D) blitting → `dma2d.rs`, driven by TouchGFX's
//!     `DMA_Interface` (cpp/N6ChromArtDMA)
//!   - LEDs → `led_service` task (green blink rate / red on-off from the GUI)
//!
//! Thread mode is donated to TouchGFX: `tgfx_task_entry()` never returns and
//! blocks in `waitForVSync` (a `wfe` loop), while the HIGH-priority
//! InterruptExecutor keeps the ticker/LED tasks running.
#![no_std]
#![no_main]

extern crate alloc;

mod bridge;
mod dma2d;

use core::sync::atomic::Ordering;

use defmt::info;
use defmt_rtt as _;
use embassy_executor::InterruptExecutor;
use embassy_stm32::interrupt;
use embassy_stm32::interrupt::{InterruptExt, Priority};
use embassy_stm32::ltdc::{LtdcLayer, LtdcLayerConfig, PixelFormat};
use embassy_time::{Duration, Ticker};
use panic_probe as _;

use bridge::{
    ffi, ANIM_ADDR, BUTTON_PTR, FB0_ADDR, FB1_ADDR, FB_BYTES, GREEN_HZ, RED_ON, TGFX_HEIGHT,
    TGFX_WIDTH, TOUCH_PTR, VSYNC, WIN_OFF_X, WIN_OFF_Y,
};
use static_cell::StaticCell;

use bsp_stm32n6570 as bsp;

// ── Panel geometry ────────────────────────────────────────────────────────────
// The GUI is full-panel 800×480, so the LTDC window fills the screen and the
// touch/view coordinate offset is zero.
const WIN_X: usize = 0;
const WIN_Y: usize = 0;

// ── Heap (required by cxx's `alloc` feature) ────
#[global_allocator]
static ALLOCATOR: alloc_cortex_m::CortexMHeap = alloc_cortex_m::CortexMHeap::empty();
const HEAP_SIZE: usize = 16 * 1024;

// ── HardFault handler — halts so the debugger can inspect the stacked frame ──
#[cortex_m_rt::exception]
unsafe fn HardFault(frame: &cortex_m_rt::ExceptionFrame) -> ! {
    defmt::error!(
        "HardFault! PC=0x{:08x} LR=0x{:08x} PSR=0x{:08x}",
        frame.pc(),
        frame.lr(),
        frame.xpsr(),
    );
    loop {
        cortex_m::asm::nop();
    }
}

// ── HIGH-priority executor (SPI6 interrupt) ───

static HIGH_EXECUTOR: InterruptExecutor = InterruptExecutor::new();

#[interrupt]
unsafe fn SPI6() {
    unsafe { HIGH_EXECUTOR.on_interrupt() }
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[cortex_m_rt::entry]
fn main() -> ! {
    // Heap first (cxx alloc support) — before anything can allocate.
    unsafe { ALLOCATOR.init(cortex_m_rt::heap_start() as usize, HEAP_SIZE) }

    // DWT cycle counter — TouchGFX's CortexMMCUInstrumentation reads CYCCNT
    // (0xE0001004) raw to compute the MCU load %, but its init() is empty: the
    // counter has to be turned on here or the load always reads 0.
    {
        let mut cp = unsafe { cortex_m::Peripherals::steal() };
        cp.DCB.enable_trace();
        cp.DWT.enable_cycle_counter();
    }

    // Clocks + embassy time driver.
    let p = bsp::clock::rcc_setup::stm32n6570_init();
    info!("clock + time driver up");

    // MPU (external memory attributes) + all SRAM banks + VddIO4 for the
    // Port Q panel-control GPIOs (LCD_ONOFF=PQ3, LCD_BL_CTRL=PQ6).
    {
        let mut core_p = unsafe { cortex_m::Peripherals::steal() };
        bsp::memory::configure_mpu_for_external_memories(&mut core_p);
    }
    // Enable all AXISRAM/AHBSRAM banks for bus masters (LTDC scans the
    // framebuffers in AXISRAM5/6) — embassy init only sets low-power gates.
    {
        use embassy_stm32::pac::RCC;
        RCC.memenr().modify(|w| {
            w.set_axisram1en(true);
            w.set_axisram2en(true);
            w.set_axisram3en(true);
            w.set_axisram4en(true);
            w.set_axisram5en(true);
            w.set_axisram6en(true);
            w.set_ahbsram1en(true);
            w.set_ahbsram2en(true);
        });
    }
    {
        use embassy_stm32::pac::PWR;
        PWR.svmcr1().modify(|w| {
            w.0 |= 1 << 8; // VDDIO4SV
        });
        cortex_m::asm::dsb();
    }

    // TouchGFX's Application ctor calls CRC_Lock() — a genuine-STM32 MCU
    // check that computes a checksum via the hardware CRC peripheral. If the
    // CRC clock is off it fails and TouchGFX will refuse to run.
    // (currentScreen = 2 → Screen::draw() through a bogus vtable → BusFault).
    embassy_stm32::pac::RCC.ahb4enr().modify(|w| w.set_crcen(true));
    cortex_m::asm::dsb();

    // ChromART (DMA2D): clock + clean IRQ state. TouchGFX's N6ChromArtDMA
    // drives it; the RIF promotion for the DMA2D bus master is already done by
    // the BSP display init. The NVIC line stays masked until TouchGFX's
    // HAL::enableInterrupts() in taskEntry.
    dma2d::init();
    info!("MPU + SRAM banks + VddIO4 + CRC + ChromART(DMA2D) ready");

    // ── PSRAM (XSPI1) — holds the 800×480 framebuffers + newlib heap ────────
    // Must be up before the framebuffers are cleared and before the C++ static
    // constructors run (their libstdc++ locale ctors sbrk into PSRAM).
    let mut ram = bsp::bsp::init_ram(
        p.XSPI1,
        p.PO4, p.PO0, p.PO2, p.PO3,
        p.PP0, p.PP1, p.PP2, p.PP3, p.PP4, p.PP5, p.PP6, p.PP7,
        p.PP8, p.PP9, p.PP10, p.PP11, p.PP12, p.PP13, p.PP14, p.PP15,
    );
    if let Err(e) = ram.init_chip() {
        defmt::panic!("PSRAM init failed: {}", e);
    }
    info!("PSRAM chip initialised (framebuffers @ {:#010x})", FB0_ADDR);

    // ── Touch driver FIRST (before the panel releases NRST) ─────────────────
    // The GT911 latches its I²C address from the INT pin level at the moment
    // reset (PE1, shared with the panel) is released. Constructing bsp_touch
    // here configures PQ4/INT the same way the demos project does — before
    // init_panel() releases NRST — so the chip latches address 0x14. Creating
    // it after panel init left INT floating and the GT911 latched 0x5D
    // ("GT911 not responding").
    let i2c2_bus = bsp::bsp::init_i2c2_bus(p.I2C2, p.PD14, p.PD4);
    let touch = bsp::bsp::init_touch(
        embassy_embedded_hal::shared_bus::blocking::i2c::I2cDevice::new(i2c2_bus),
        p.PQ4,
        p.EXTI4,
    );
    static TOUCH: StaticCell<bsp_stm32n6570::touch::bsp_touch> = StaticCell::new();
    let touch: &'static mut bsp_stm32n6570::touch::bsp_touch = TOUCH.init(touch);

    // ── Display: LTDC timings + panel power ─────────────────────────────────
    let mut display = bsp::bsp::init_display(
        p.LTDC,
        // CLK, HSYNC, VSYNC, DE
        p.PB13, p.PB14, p.PE11, p.PG13,
        // R0-R7
        p.PG0, p.PD9, p.PD15, p.PB4, p.PH4, p.PA15, p.PG11, p.PD8,
        // G0-G7
        p.PG12, p.PG1, p.PA1, p.PA0, p.PB15, p.PB12, p.PB11, p.PG8,
        // B0-B7
        p.PG15, p.PA7, p.PB2, p.PG6, p.PH3, p.PH6, p.PA8, p.PA2,
        // Control GPIOs: BL_CTRL, LCD_ONOFF, LCD_NRST
        p.PQ6, p.PQ3, p.PE1,
        bsp::bsp_pixel_format::RGB565,
    );
    display.init_panel();

    // Clear both framebuffers + animation storage before scan-out starts.
    unsafe {
        core::ptr::write_bytes(FB0_ADDR as *mut u8, 0, FB_BYTES);
        core::ptr::write_bytes(FB1_ADDR as *mut u8, 0, FB_BYTES);
        core::ptr::write_bytes(ANIM_ADDR as *mut u8, 0, FB_BYTES);
    }
    cortex_m::asm::dsb();

    // LTDC Layer1: full-screen 800×480 RGB565, scanning FB0 in PSRAM.
    let layer_cfg = LtdcLayerConfig {
        layer: LtdcLayer::Layer1,
        pixel_format: PixelFormat::RGB565,
        window_x0: 0,
        window_x1: TGFX_WIDTH as u16,
        window_y0: 0,
        window_y1: TGFX_HEIGHT as u16,
    };
    {
        let ltdc = display.get_ltdc();
        ltdc.init_layer(&layer_cfg, None);
        ltdc.init_buffer(LtdcLayer::Layer1, FB0_ADDR as *const ());
        embassy_stm32::pac::LTDC.srcr().write(|w| {
            w.set_imr(embassy_stm32::pac::ltdc::vals::Imr::Reload);
        });
    }
    info!("LTDC Layer1: {}x{} full-screen, FB0={:#010x}", TGFX_WIDTH, TGFX_HEIGHT, FB0_ADDR);

    // ── GT911 chip bring-up (panel NRST is high now, give it boot time) ─────
    embassy_time::block_for(Duration::from_millis(50));
    touch.init_chip();
    match touch.read_product_id() {
        Ok(id) => info!("GT911 product ID: {=[u8]:a}", id),
        Err(_) => info!("GT911 not responding — touch disabled"),
    }
    // Hand the driver to the bridge's sampleTouch (thread-mode only).
    WIN_OFF_X.store(WIN_X as i32, Ordering::Relaxed);
    WIN_OFF_Y.store(WIN_Y as i32, Ordering::Relaxed);
    TOUCH_PTR.store(touch as *mut _ as u32, Ordering::Relaxed);

    // ── USER button (PC13) → TouchGFX ButtonController ──────────────────────
    // Sampled from thread mode by the C++ N6ButtonController each tick.
    let buttons = bsp::bsp::init_buttons(p.PC13, p.EXTI13);
    static BUTTONS: StaticCell<bsp_stm32n6570::buttons::bsp_buttons> = StaticCell::new();
    let buttons: &'static mut bsp_stm32n6570::buttons::bsp_buttons = BUTTONS.init(buttons);
    BUTTON_PTR.store(buttons as *mut _ as u32, Ordering::Relaxed);

    // ── Board LEDs → owned by led_service (green blink = slider, red = toggle) ─
    let leds = bsp::bsp::init_leds(p.PO1, p.PG10);

    // ── HIGH executor: vsync ticker + LED service ───────────────────────────
    interrupt::SPI6.set_priority(Priority::P7);
    let spawner = HIGH_EXECUTOR.start(interrupt::SPI6);
    spawner.spawn(vsync_ticker().expect("spawn vsync"));
    spawner.spawn(led_service(leds).expect("spawn led_service"));

    // ── C++ static constructors (HAL, LCD16bpp, fonts, …) ────────────────────
    run_cpp_constructors();
    info!("C++ static constructors done");

    // ── TouchGFX up ──────────────────────────────────────────────────────────
    ffi::tgfx_init();
    info!("TouchGFX initialised — entering render loop");
    ffi::tgfx_task_entry();
    // tgfx_task_entry never returns.
    unreachable!()
}

/// Walk `.init_array` (collected by touchgfx_ctors.x) and run every C++
/// static constructor. Must run before any TouchGFX code executes.
fn run_cpp_constructors() {
    unsafe extern "C" {
        static __init_array_start: extern "C" fn();
        static __init_array_end: extern "C" fn();
    }
    unsafe {
        let mut f = &raw const __init_array_start;
        let end = &raw const __init_array_end;
        while f < end {
            (*f)();
            f = f.add(1);
        }
    }
}

// ── HIGH-priority tasks ───────────────────────────────────────────────────────

/// ~60 Hz vsync heartbeat for the TouchGFX render loop.
#[embassy_executor::task]
async fn vsync_ticker() {
    let mut ticker = Ticker::every(Duration::from_millis(17));
    loop {
        ticker.next().await;
        VSYNC.store(true, Ordering::Release);
        // Thread mode sleeps in `wfe`; the ticker's timer interrupt already
        // woke it, `sev` just makes the wake explicit.
        cortex_m::asm::sev();
    }
}

/// Owns the board LEDs. Green blinks at `GREEN_HZ` (0..100 Hz, 0 = off, set by
/// the GUI slider); red mirrors `RED_ON` (toggle / USER button). Runs on a
/// fixed 5 ms base tick (200 Hz) so it can reach a 100 Hz blink and still
/// apply red changes promptly, while keeping single ownership of `bsp_leds`.
#[embassy_executor::task]
async fn led_service(mut leds: bsp_stm32n6570::leds::bsp_leds) {
    use bsp_stm32n6570::leds::LEDs;

    const TICK_MS: u32 = 5;
    let mut ticker = Ticker::every(Duration::from_millis(TICK_MS as u64));
    let mut green_on = false;
    let mut acc_ms: u32 = 0;

    loop {
        ticker.next().await;

        // Red: direct mirror of the GUI toggle.
        if RED_ON.load(Ordering::Relaxed) {
            leds.on(LEDs::USER_LED_RED);
        } else {
            leds.off(LEDs::USER_LED_RED);
        }

        // Green: blink at GREEN_HZ (half-period = 1000 / (2*hz) ms).
        let hz = GREEN_HZ.load(Ordering::Relaxed);
        if hz == 0 {
            if green_on {
                leds.off(LEDs::USER_LED_GREEN);
                green_on = false;
            }
            acc_ms = 0;
        } else {
            let half_period_ms = (1000 / (2 * hz)).max(TICK_MS);
            acc_ms += TICK_MS;
            if acc_ms >= half_period_ms {
                acc_ms = 0;
                green_on = !green_on;
                if green_on {
                    leds.on(LEDs::USER_LED_GREEN);
                } else {
                    leds.off(LEDs::USER_LED_GREEN);
                }
            }
        }
    }
}
