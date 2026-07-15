// C++ entry points exposed to Rust through the cxx bridge (src/bridge.rs).
#pragma once

// Construct + initialize the TouchGFX framework (bitmap DB, texts, fonts,
// HAL). C++ static constructors must have run first (main.rs walks
// .init_array before calling this).
void tgfx_init();

// TouchGFX main loop: waitForVSync() -> tick -> render, forever.
// OSWrappers::waitForVSync() bridges back into Rust, which paces the loop
// off the vsync ticker task. Never returns.
void tgfx_task_entry();

// Called from the Rust DMA2D ISR when a ChromART blit completes: advances
// TouchGFX's blit queue (HAL -> DMA_Interface::signalDMAInterrupt).
void tgfx_signal_dma_irq();

// ── Diagnostics, used by the GUI (Screen1View) ──────────────────────────────
// Runtime ChromART on/off. Disabled => getBlitCaps() returns 0 => TouchGFX
// software-renders everything.
void tgfx_set_dma_enabled(bool enabled);
// MCU load 0..100 % (TouchGFX's DWT-based measurement).
unsigned char tgfx_get_mcu_load_pct();
