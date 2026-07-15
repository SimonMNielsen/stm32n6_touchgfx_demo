#include "N6TouchGFXHAL.hpp"

#include <touchgfx/Application.hpp>

// cxx-generated declarations of the Rust bridge functions.
#include "stm32n6_touchgfx_demo/src/bridge.rs.h"

using namespace touchgfx;

void N6TouchGFXHAL::initialize()
{
    HAL::initialize();
    registerEventListener(*(Application::getInstance()));

    // Double-buffered rendering: buffers live in AXISRAM5/6, owned by Rust.
    setFrameBufferStartAddresses((void*)(uintptr_t)rust_fb0_addr(),
                                 (void*)(uintptr_t)rust_fb1_addr(),
                                 (void*)0);

    // Animation storage (slide transitions etc.) — third AXISRAM buffer.
    setAnimationStorage((void*)(uintptr_t)rust_anim_addr());
}

void N6TouchGFXHAL::configureInterrupts()
{
    rust_dma2d_configure_irq();
}

void N6TouchGFXHAL::enableInterrupts()
{
    rust_dma2d_enable_irq();
}

void N6TouchGFXHAL::disableInterrupts()
{
    rust_dma2d_disable_irq();
}

uint16_t* N6TouchGFXHAL::getTFTFrameBuffer() const
{
    return (uint16_t*)(uintptr_t)rust_get_visible_framebuffer();
}

void N6TouchGFXHAL::setTFTFrameBuffer(uint16_t* adr)
{
    rust_set_visible_framebuffer((uint32_t)(uintptr_t)adr);
}
