// Application-only callbacks used by the generated GUI behavior.
#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

void rust_set_green_hz(uint8_t hz);
void rust_set_red(bool on);

#ifdef __cplusplus
}

// Application functions implemented by TouchGFXConfiguration.cpp.
void tgfx_set_dma_enabled(bool enabled);
unsigned char tgfx_get_mcu_load_pct();
#endif
