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

    // These gate the ChromART blit-complete IRQ. TouchGFX brackets its
    // blit-queue updates with disable/enableInterrupts, so these MUST really
    // mask the DMA2D line — leaving them as no-ops lets the ISR pop from the
    // queue while thread mode is pushing to it.
    virtual void configureInterrupts();
    virtual void enableInterrupts();
    virtual void disableInterrupts();

    // Frame pacing comes from the Rust vsync ticker, not an LTDC line IRQ.
    virtual void enableLCDControllerInterrupt() {}

protected:
    virtual uint16_t* getTFTFrameBuffer() const;
    virtual void setTFTFrameBuffer(uint16_t* adr);
};
