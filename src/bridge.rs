//! App-specific TouchGFX bridge implementation for the STM32N6570-DK demo.
//!
//! Only board-specific callbacks live here: framebuffer address getters,
//! GT911 touch sampling, USER-button sampling, and LED-state publishing.
//! Reusable bridge callbacks (vsync signalling, `taskDelay`, LTDC layer-0
//! swap, DMA2D shims) live in [`touchgfx_rs::runtime`] and
//! [`touchgfx_rs::bridge`].

use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

// ── Application framebuffer geometry/layout ─────────────────────────────────

pub const TGFX_WIDTH: usize = 800;
pub const TGFX_HEIGHT: usize = 480;
pub const FB_BYTES: usize = TGFX_WIDTH * TGFX_HEIGHT * 2;

// FB0 + FB1 live in AXISRAM (internal, multi-GB/s) so that every ChromART
// blit hits fast SRAM instead of the xSPI1 PSRAM bus at 133 MHz. The demo's
// `memory_ram.x` reserves only 0x34000000..0x34200000 for code + stack, so
// 0x34200000 upward is free. ST's N6570-DK reference project uses this same
// region for `FB_RAM` (see touchgfx_project/Appli/STM32N657XX_LRUN.ld).
// FB0 = 800*480*2 = 750000 B (~732 KiB), FB1 same → ~1.5 MiB total.
pub const FB0_ADDR: u32 = 0x3420_0000;
pub const FB1_ADDR: u32 = FB0_ADDR + FB_BYTES as u32;
// Animation storage is only touched during screen transitions. Keep it in
// PSRAM to bound the internal-SRAM footprint.
pub const ANIM_ADDR: u32 = 0x9000_0000;

// ── Shared state (bridge callbacks ↔ embassy tasks) ──────────────────────────

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

// ── extern "Rust" implementations ────────────────────────────────────────────

/// Publish the green-LED blink rate (Hz, 0..100). The `led_service` task
/// picks it up and does the timing.
#[no_mangle]
pub extern "C" fn rust_set_green_hz(hz: u8) {
    GREEN_HZ.store(hz as u32, Ordering::Relaxed);
}

/// Publish the red-LED on/off state.
#[no_mangle]
pub extern "C" fn rust_set_red(on: bool) {
    RED_ON.store(on, Ordering::Relaxed);
}

/// USER-button sample for the TouchGFX ButtonController: reports key `1`
/// exactly once per press (edge detection here keeps the GUI's
/// handleKeyEvent from repeating while the button is held).
#[no_mangle]
pub unsafe extern "C" fn rust_button_sample(key: *mut u8) -> bool {
    use bsp_stm32n6570::buttons::bsp_buttons;

    static WAS_PRESSED: AtomicBool = AtomicBool::new(false);

    if key.is_null() {
        return false;
    }

    let ptr = BUTTON_PTR.load(Ordering::Relaxed) as *mut bsp_buttons;
    if ptr.is_null() {
        return false;
    }
    // Safety: set once in main; only this (thread-mode) function dereferences.
    let buttons = unsafe { &mut *ptr };

    let pressed = buttons.is_pressed();
    let was = WAS_PRESSED.swap(pressed, Ordering::Relaxed);
    if pressed && !was {
        unsafe { *key = 1 };
        true
    } else {
        false
    }
}

#[no_mangle]
pub extern "C" fn rust_fb0_addr() -> u32 {
    FB0_ADDR
}

#[no_mangle]
pub extern "C" fn rust_fb1_addr() -> u32 {
    FB1_ADDR
}

#[no_mangle]
pub extern "C" fn rust_anim_addr() -> u32 {
    ANIM_ADDR
}

/// One GT911 sample in view coordinates; true while the panel is touched
/// inside the 800x480 TouchGFX window.
///
/// Called by TouchGFX (thread mode) once per tick — the blocking I²C read
/// (~1 ms) happens at the GUI's own rate. The GT911 only latches *new*
/// samples, so a short miss-tolerance keeps drags from flickering to
/// "released" between reports.
#[no_mangle]
pub unsafe extern "C" fn rust_touch_sample(x: *mut i32, y: *mut i32) -> bool {
    use bsp_stm32n6570::touch::bsp_touch;

    if x.is_null() || y.is_null() {
        return false;
    }

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
        unsafe {
            *x = LAST_X.load(Ordering::Relaxed);
            *y = LAST_Y.load(Ordering::Relaxed);
        }
        true
    } else {
        false
    }
}
