// USER-button (PC13) controller: samples come from the BSP button driver in
// Rust, read through the cxx bridge. TouchGFX polls sample() once per tick
// and delivers key events to the active Screen's handleKeyEvent().
#pragma once

#include <platform/driver/button/ButtonController.hpp>

class N6ButtonController : public touchgfx::ButtonController
{
public:
    // PC13 bring-up is done on the Rust side — nothing to init here.
    virtual void init() {}

    virtual bool sample(uint8_t& key);
};
