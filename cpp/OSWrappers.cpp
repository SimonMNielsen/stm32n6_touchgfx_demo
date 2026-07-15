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
    // This is the GUI's idle: thread mode sleeps in `wfe` here until the next
    // vsync tick. TouchGFX measures MCU load by having the *idle* path bracket
    // itself with setMCUActive(false/true) — in ST's reference that's the
    // FreeRTOS idle hook. With no RTOS, nothing else can do it, and without
    // these two calls cc_consumed never accumulates and the load reads a
    // constant 100 %.
    HAL* hal = HAL::getInstance();
    if (hal != 0)
    {
        hal->setMCUActive(false); // idle begins
    }

    rust_wait_for_vsync();

    if (hal != 0)
    {
        hal->setMCUActive(true); // idle ends
    }
}

void OSWrappers::taskDelay(uint16_t ms)
{
    rust_delay_ms(ms);
}

void OSWrappers::taskYield() {}
