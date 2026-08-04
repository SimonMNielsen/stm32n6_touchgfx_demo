// Application-only C ABI: Rust callbacks consumed by the TouchGFX Designer-
// generated GUI code (currently Screen1View.cpp). These are inherently GUI
// policy — the LED-service task in Rust owns the actual hardware, this
// header only lets the GUI publish the desired state.
//
// The generic adapter classes provided by touchgfx-rs (touch controller,
// button controller, HAL) require additional Rust callbacks — those are
// declared inside the crate's own C++ and satisfied by the application's
// Rust bridge module. This header does not need to redeclare them.
#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// GUI → app publishers.
void rust_set_green_hz(uint8_t hz);
void rust_set_red(bool on);

#ifdef __cplusplus
}

// Application functions implemented by TouchGFXConfiguration.cpp.
void tgfx_set_dma_enabled(bool enabled);
unsigned char tgfx_get_mcu_load_pct();
#endif
