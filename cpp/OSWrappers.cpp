// TouchGFX OS abstraction for the no-RTOS (embassy) build.
//
// Same NoOS shape as the original project's OSWrappers.cpp (all stubs), but
// waitForVSync/taskDelay bridge into Rust so the render loop is paced by the
// embassy vsync ticker instead of free-running.

#include <touchgfx/hal/HAL.hpp>
#include <touchgfx/hal/OSWrappers.hpp>

#include "stm32n6_touchgfx_demo/src/bridge.rs.h"

using namespace touchgfx;

void OSWrappers::initialize() {}

// Single-threaded GUI: no other task ever touches the framebuffer, so the
// framebuffer semaphore can be a no-op (same as the original NoOS project).
void OSWrappers::takeFrameBufferSemaphore() {}
void OSWrappers::giveFrameBufferSemaphore() {}
void OSWrappers::tryTakeFrameBufferSemaphore() {}
void OSWrappers::giveFrameBufferSemaphoreFromISR() {}

// The vsync signal originates in Rust (embassy ticker task) — the C++ side
// only ever consumes it in waitForVSync().
void OSWrappers::signalVSync() {}
void OSWrappers::signalRenderingDone() {}

void OSWrappers::waitForVSync()
{
    rust_wait_for_vsync();
}

void OSWrappers::taskDelay(uint16_t ms)
{
    rust_delay_ms(ms);
}

void OSWrappers::taskYield() {}
