//! cxx bridge between the TouchGFX C++ glue (cpp/) and the embassy firmware.
//!
//! Direction C++ → Rust: vsync pacing, framebuffer swap (LTDC), touch
//! samples, delays. Direction Rust → C++: framework init + the render loop.
//!
//! Everything crossing the bridge is a primitive, so the generated code
//! needs no cxx runtime support (no std, no alloc).

use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

use embassy_time::{Duration, Instant};

// ── Framebuffer layout (PSRAM — 800×480 full-panel, RGB565) ─────────────────

/// The N6 TouchGFX app is full-screen 800×480. Software rendering (LCD16bpp)
/// writes into a double framebuffer + animation storage in PSRAM: each frame
/// is 750 KB, too big for the AXISRAM slots the H750 build used, and PSRAM is
/// non-cacheable so the LTDC sees CPU writes with no cache maintenance.
pub const TGFX_WIDTH: usize = 800;
pub const TGFX_HEIGHT: usize = 480;
pub const FB_BYTES: usize = TGFX_WIDTH * TGFX_HEIGHT * 2; // 768000

pub const FB0_ADDR: u32 = 0x9000_0000;
pub const FB1_ADDR: u32 = 0x900C_0000;
pub const ANIM_ADDR: u32 = 0x9018_0000;

// ── Shared state (bridge fns ↔ embassy tasks) ────────────────────────────────

/// Set by the vsync ticker task (HIGH executor), consumed by
/// `rust_wait_for_vsync` on the TouchGFX (thread-mode) side.
pub static VSYNC: AtomicBool = AtomicBool::new(false);

/// LTDC Layer1 CFBAR shadow — which buffer is currently being scanned out.
pub static VISIBLE_FB: AtomicU32 = AtomicU32::new(FB0_ADDR);

/// GT911 handle for thread-mode sampling. Set once by `main` before TouchGFX
/// starts; only ever dereferenced from `rust_touch_sample`, which TouchGFX
/// calls from thread mode — no concurrent access exists.
pub static TOUCH_PTR: AtomicU32 = AtomicU32::new(0);

/// USER-button (PC13) handle for thread-mode sampling by the TouchGFX
/// ButtonController. Same single-context contract as [`TOUCH_PTR`].
pub static BUTTON_PTR: AtomicU32 = AtomicU32::new(0);

/// Green-LED blink frequency in Hz (0 = off), set from the GUI slider.
/// The `led_service` task owns the LED GPIOs and does the actual blinking,
/// so the GUI only ever publishes state here — no shared `&mut bsp_leds`.
pub static GREEN_HZ: AtomicU32 = AtomicU32::new(0);
/// Red-LED on/off, set from the GUI toggle / USER button.
pub static RED_ON: AtomicBool = AtomicBool::new(false);

/// TouchGFX view window offset on the 800×480 panel (set by `main`).
pub static WIN_OFF_X: AtomicI32 = AtomicI32::new(0);
pub static WIN_OFF_Y: AtomicI32 = AtomicI32::new(0);

#[cxx::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("touchgfx_shim.h");

        /// Initialize the TouchGFX framework (after C++ static ctors ran).
        fn tgfx_init();

        /// TouchGFX main loop — never returns.
        fn tgfx_task_entry();
    }

    extern "Rust" {
        fn rust_wait_for_vsync();
        fn rust_delay_ms(ms: u16);
        fn rust_button_sample(key: &mut u8) -> bool;
        /// Green LED (PO1) blink rate, 0..100 Hz (0 = off).
        fn rust_set_green_hz(hz: u8);
        /// Red LED (PG10) on/off.
        fn rust_set_red(on: bool);
        fn rust_fb0_addr() -> u32;
        fn rust_fb1_addr() -> u32;
        fn rust_anim_addr() -> u32;
        fn rust_get_visible_framebuffer() -> u32;
        fn rust_set_visible_framebuffer(addr: u32);
        fn rust_touch_sample(x: &mut i32, y: &mut i32) -> bool;
    }
}

// ── extern "Rust" implementations ────────────────────────────────────────────

/// Publish the green-LED blink rate (Hz, 0..100). The `led_service` task
/// picks it up and does the timing.
fn rust_set_green_hz(hz: u8) {
    GREEN_HZ.store(hz as u32, Ordering::Relaxed);
}

/// Publish the red-LED on/off state.
fn rust_set_red(on: bool) {
    RED_ON.store(on, Ordering::Relaxed);
}

/// USER-button sample for the TouchGFX ButtonController: reports key `1`
/// exactly once per press (edge detection here keeps the GUI's
/// handleKeyEvent from repeating while the button is held).
fn rust_button_sample(key: &mut u8) -> bool {
    use bsp_stm32n6570::buttons::bsp_buttons;

    static WAS_PRESSED: AtomicBool = AtomicBool::new(false);

    let ptr = BUTTON_PTR.load(Ordering::Relaxed) as *mut bsp_buttons;
    if ptr.is_null() {
        return false;
    }
    // Safety: set once in main; only this (thread-mode) function dereferences.
    let buttons = unsafe { &mut *ptr };

    let pressed = buttons.is_pressed();
    let was = WAS_PRESSED.swap(pressed, Ordering::Relaxed);
    if pressed && !was {
        *key = 1;
        true
    } else {
        false
    }
}

/// Block (thread mode) until the vsync ticker fires. Any interrupt wakes the
/// `wfe`, so the HIGH-priority executor keeps running while we sleep here.
fn rust_wait_for_vsync() {
    while !VSYNC.swap(false, Ordering::Acquire) {
        cortex_m::asm::wfe();
    }
}

/// Blocking millisecond delay (TouchGFX `OSWrappers::taskDelay`).
fn rust_delay_ms(ms: u16) {
    let deadline = Instant::now() + Duration::from_millis(ms as u64);
    while Instant::now() < deadline {
        cortex_m::asm::nop();
    }
}

fn rust_fb0_addr() -> u32 {
    FB0_ADDR
}

fn rust_fb1_addr() -> u32 {
    FB1_ADDR
}

fn rust_anim_addr() -> u32 {
    ANIM_ADDR
}

fn rust_get_visible_framebuffer() -> u32 {
    VISIBLE_FB.load(Ordering::Relaxed)
}

/// Point LTDC Layer1 at `addr` with an immediate reload — the TouchGFX
/// double-buffer swap (equivalent of the H7 project's `LTDC_Layer1->CFBAR =`).
fn rust_set_visible_framebuffer(addr: u32) {
    use embassy_stm32::pac::ltdc::vals::Imr;
    use embassy_stm32::pac::LTDC;

    LTDC.layer(0).cfbar().write(|w| w.set_cfbadd(addr));
    LTDC.srcr().write(|w| w.set_imr(Imr::Reload));
    VISIBLE_FB.store(addr, Ordering::Relaxed);
}

/// One GT911 sample in view coordinates; true while the panel is touched
/// inside the 800x480 TouchGFX window.
///
/// Called by TouchGFX (thread mode) once per tick — the blocking I²C read
/// (~1 ms) happens at the GUI's own rate. The GT911 only latches *new*
/// samples, so a short miss-tolerance keeps drags from flickering to
/// "released" between reports.
fn rust_touch_sample(x: &mut i32, y: &mut i32) -> bool {
    use bsp_stm32n6570::touch::bsp_touch;

    // Last reported state (thread-mode only — plain statics via atomics).
    static LAST_DOWN: AtomicBool = AtomicBool::new(false);
    static LAST_X: AtomicI32 = AtomicI32::new(0);
    static LAST_Y: AtomicI32 = AtomicI32::new(0);
    static MISSES: AtomicU32 = AtomicU32::new(0);
    const MISS_LIMIT: u32 = 3; // ~3 ticks (50 ms) without data → finger up

    let ptr = TOUCH_PTR.load(Ordering::Relaxed) as *mut bsp_touch;
    if ptr.is_null() {
        return false;
    }
    // Safety: set once in main; only this (thread-mode) function dereferences.
    let touch = unsafe { &mut *ptr };

    match touch.detect_touch() {
        Ok(Some(n)) if n > 0 => {
            if let Ok(tp) = touch.get_touch() {
                let vx = tp.x as i32 - WIN_OFF_X.load(Ordering::Relaxed);
                let vy = tp.y as i32 - WIN_OFF_Y.load(Ordering::Relaxed);
                if vx >= 0 && vx < TGFX_WIDTH as i32 && vy >= 0 && vy < TGFX_HEIGHT as i32 {
                    LAST_X.store(vx, Ordering::Relaxed);
                    LAST_Y.store(vy, Ordering::Relaxed);
                    LAST_DOWN.store(true, Ordering::Relaxed);
                } else {
                    LAST_DOWN.store(false, Ordering::Relaxed);
                }
                MISSES.store(0, Ordering::Relaxed);
            }
        }
        Ok(_) => {
            // No new data — clear the status register and count the miss.
            let _ = touch.get_touches();
            let m = MISSES.load(Ordering::Relaxed) + 1;
            MISSES.store(m, Ordering::Relaxed);
            if m >= MISS_LIMIT {
                LAST_DOWN.store(false, Ordering::Relaxed);
            }
        }
        Err(_) => {
            let m = MISSES.load(Ordering::Relaxed) + 1;
            MISSES.store(m, Ordering::Relaxed);
            if m >= MISS_LIMIT {
                LAST_DOWN.store(false, Ordering::Relaxed);
            }
        }
    }

    if LAST_DOWN.load(Ordering::Relaxed) {
        *x = LAST_X.load(Ordering::Relaxed);
        *y = LAST_Y.load(Ordering::Relaxed);
        true
    } else {
        false
    }
}
