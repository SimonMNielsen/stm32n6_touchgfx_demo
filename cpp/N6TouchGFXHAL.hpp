// TouchGFX HAL for the STM32N6570-DK with all board access bridged to Rust.
//
// Replaces the generated TouchGFXGeneratedHAL/TouchGFXHAL pair from the
// original H750 project: no ST HAL, no direct register access — LTDC frame
// buffer swaps go through the cxx bridge into embassy-stm32.
#pragma once

#include <touchgfx/hal/HAL.hpp>

class N6TouchGFXHAL : public touchgfx::HAL
{
public:
    N6TouchGFXHAL(touchgfx::DMA_Interface& dma,
                  touchgfx::LCD& display,
                  touchgfx::TouchController& tc,
                  uint16_t width,
                  uint16_t height)
        : touchgfx::HAL(dma, display, tc, width, height)
    {
    }

    void initialize();

    // Interrupt management is owned by Rust/embassy — nothing to do here.
    virtual void configureInterrupts() {}
    virtual void enableInterrupts() {}
    virtual void disableInterrupts() {}
    virtual void enableLCDControllerInterrupt() {}

protected:
    virtual uint16_t* getTFTFrameBuffer() const;
    virtual void setTFTFrameBuffer(uint16_t* adr);
};
