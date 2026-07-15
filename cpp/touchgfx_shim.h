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
