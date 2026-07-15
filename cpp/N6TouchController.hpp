// Touch controller shim: samples come from the GT911 driver in the Rust BSP
// (polled by an embassy task), read here through the cxx bridge.
#pragma once

#include <platform/driver/touch/TouchController.hpp>

class N6TouchController : public touchgfx::TouchController
{
public:
    // GT911 bring-up is done on the Rust side (needs the panel-reset
    // sequencing owned by the display driver) — nothing to init here.
    virtual void init() {}

    virtual bool sampleTouch(int32_t& x, int32_t& y);
};
