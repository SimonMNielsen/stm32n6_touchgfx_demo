//! TouchGFX on Rust — STM32N6570-DK.
//!
//! The C++ TouchGFX application vendored in `touchgfx_project/` (800×480
//! RGB565, no RTOS) runs on this board with **all** hardware access going
//! through the Rust N6 BSP + embassy — no ST HAL anywhere:
//!
//!   - LTDC / panel / backlight  → `bsp_stm32n6570::display`, full-screen
//!   - framebuffers (FB0/FB1 in AXISRAM, animation storage in PSRAM) +
//!     double-buffer swap → C ABI → embassy PAC
//!   - vsync pacing → embassy ticker task on the HIGH interrupt executor
//!   - touch → GT911, sampled from thread mode over the bridge
//!   - ChromART (DMA2D) blitting → `touchgfx-rs`, driven by its reusable
//!     `DMA_Interface`
//!   - LEDs → `led_service` task (green blink rate / red on-off from the GUI)
//!
//! Thread mode is donated to TouchGFX: `tgfx_task_entry()` never returns and
//! blocks in `waitForVSync` (a `wfe` loop), while the HIGH-priority
//! InterruptExecutor keeps the ticker/LED tasks running.
#![no_std]
#![no_main]

extern crate alloc;

mod bridge;
mod clock;

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
    ANIM_ADDR, BUTTON_PTR, FB0_ADDR, FB1_ADDR, FB_BYTES, GREEN_HZ, RED_ON, TGFX_HEIGHT,
    TGFX_WIDTH, TOUCH_PTR, WIN_OFF_X, WIN_OFF_Y,
};
use touchgfx_rs::ffi;
use touchgfx_rs::runtime;
use static_cell::StaticCell;

use bsp_stm32n6570 as bsp;

// ── Panel geometry ────────────────────────────────────────────────────────────
// The GUI is full-panel 800×480, so the LTDC window fills the screen and the
// touch/view coordinate offset is zero.
const WIN_X: usize = 0;
const WIN_Y: usize = 0;

// ── Rust heap ────────────────────────────────────────────────────────────────
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
    // Heap first — before any Rust component can allocate.
    unsafe { ALLOCATOR.init(cortex_m_rt::heap_start() as usize, HEAP_SIZE) }

    // DWT cycle counter — TouchGFX's CortexMMCUInstrumentation reads CYCCNT
    // (0xE0001004) raw to compute the MCU load %, but its init() is empty: the
    // counter has to be turned on here or the load always reads 0.
    runtime::enable_dwt_cycle_counter();

    // Clocks + embassy time driver. `clock::init` extends the BSP baseline
    // clock tree with the IC2/IC6/IC11 system-bus group this app needs.
    let p = clock::init();
    info!("clock + time driver up");

    // ── BSP subsystems in one call ───────────────────────────────────────────
    // `init_hardware` configures the MPU for external memories, enables the
    // VddIO4 supply valid (needed for Port Q GPIOs: LCD_ONOFF=PQ3,
    // LCD_BL_CTRL=PQ6), and brings up every subsystem requested below.
    //
    // Deferred to the app (still done further down): PSRAM chip init,
    // panel init (releases LCD_NRST), GT911 chip init.
    let hw = bsp::init::init_hardware(
        p,
        bsp::init::InitConfig {
            leds:    true,
            buttons: true,
            touch:   true,   // MCU-side only — chip init done after panel NRST release
            display: true,   // MCU-side only — init_panel() done after LTDC layer setup
            imu:     false,
            flash:   false,  // no NOR flash — RAM/probe-rs boot
            ram:     true,   // MCU-side only — chip init done below
            tof:     false,
            camera:  false,
        },
    );
    let mut ram     = hw.ram    .expect("BSP: PSRAM handle missing");
    let mut display = hw.display.expect("BSP: display handle missing");
    let touch       = hw.touch  .expect("BSP: touch handle missing");
    let buttons     = hw.buttons.expect("BSP: buttons handle missing");
    let leds        = hw.leds   .expect("BSP: leds handle missing");

    // Enable all AXISRAM/AHBSRAM banks for bus masters (LTDC + DMA2D scan
    // the framebuffers in AXISRAM at 0x3420_0000) — embassy init only sets
    // low-power gates, and the BSP intentionally doesn't touch MEMENR.
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

    // TouchGFX's Application ctor calls CRC_Lock() — needs the CRC clock.
    runtime::enable_crc_clock();

    // ChromART (DMA2D): clock + clean IRQ state. The reusable DMA interface
    // drives it; the RIF promotion for the DMA2D bus master is already done by
    // the BSP display init. The NVIC line stays masked until TouchGFX's
    // HAL::enableInterrupts() in taskEntry.
    touchgfx_rs::dma2d::init();
    info!("SRAM banks + CRC + ChromArt(DMA2D) ready");

    // ── PSRAM chip init — holds the animation-storage buffer + newlib heap.
    //    FB0/FB1 live in AXISRAM (see bridge.rs); only ANIM_ADDR is in PSRAM.
    //    Must be up before the framebuffers are cleared and TouchGFX starts.
    if let Err(e) = ram.init_chip() {
        defmt::panic!("PSRAM init failed: {}", e);
    }
    info!("PSRAM chip initialised (FB0 in AXISRAM @ {:#010x}, ANIM in PSRAM @ {:#010x})", FB0_ADDR, ANIM_ADDR);

    // ── Touch: stash the driver into a StaticCell — chip init runs after
    // panel NRST is released (below) so the GT911 has already latched I²C
    // address 0x14 from the INT pin level at reset. See BSP init_hardware:
    // touch MCU-side is initialised BEFORE display MCU-side (which puts
    // LCD_NRST=PE1 low), and BEFORE `display.init_panel()` releases it,
    // so PQ4/INT is driven the whole time NRST transitions high.
    static TOUCH: StaticCell<bsp_stm32n6570::touch::bsp_touch> = StaticCell::new();
    let touch: &'static mut bsp_stm32n6570::touch::bsp_touch = TOUCH.init(touch);

    // ── Display: release panel NRST (LCD_NRST=PE1 goes high). ────────────────
    display.init_panel();

    // Clear both framebuffers + animation storage before scan-out starts.
    unsafe {
        core::ptr::write_bytes(FB0_ADDR as *mut u8, 0, FB_BYTES);
        core::ptr::write_bytes(FB1_ADDR as *mut u8, 0, FB_BYTES);
        core::ptr::write_bytes(ANIM_ADDR as *mut u8, 0, FB_BYTES);
    }
    cortex_m::asm::dsb();

    // LTDC Layer1: full-screen 800×480 RGB565, scanning FB0 in AXISRAM.
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
    // Tell the touchgfx-rs LTDC swap layer which framebuffer is currently
    // being scanned out (matches the `init_buffer` call above).
    runtime::visible_fb::init(FB0_ADDR);
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
    // Sampled from thread mode by the RustBridgedButtonController each tick.
    // `buttons` came from `init_hardware` above.
    static BUTTONS: StaticCell<bsp_stm32n6570::buttons::bsp_buttons> = StaticCell::new();
    let buttons: &'static mut bsp_stm32n6570::buttons::bsp_buttons = BUTTONS.init(buttons);
    BUTTON_PTR.store(buttons as *mut _ as u32, Ordering::Relaxed);

    // ── Board LEDs → owned by led_service (green blink = slider, red = toggle) ─
    // `leds` came from `init_hardware` above.

    // ── HIGH executor: vsync ticker + LED service ───────────────────────────
    interrupt::SPI6.set_priority(Priority::P7);
    let spawner = HIGH_EXECUTOR.start(interrupt::SPI6);
    spawner.spawn(vsync_ticker().expect("spawn vsync"));
    spawner.spawn(led_service(leds).expect("spawn led_service"));

    // ── C++ static constructors (HAL, LCD16bpp, fonts, …) ────────────────────
    runtime::run_cpp_constructors();
    info!("C++ static constructors done");

    // ── TouchGFX up ──────────────────────────────────────────────────────────
    ffi::tgfx_init();
    info!("TouchGFX initialised — entering render loop");
    ffi::tgfx_task_entry();
    // tgfx_task_entry never returns.
    unreachable!()
}

// ── HIGH-priority tasks ───────────────────────────────────────────────────────

/// ~60 Hz vsync heartbeat for the TouchGFX render loop.
#[embassy_executor::task]
async fn vsync_ticker() {
    let mut ticker = Ticker::every(Duration::from_millis(17));
    loop {
        ticker.next().await;
        runtime::vsync::signal();
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
