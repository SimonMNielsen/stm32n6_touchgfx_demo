#include "touchgfx_shim.h"

#include <touchgfx/hal/HAL.hpp>

extern "C" void touchgfx_init();
extern "C" void touchgfx_taskEntry();

void tgfx_init()
{
    touchgfx_init();
}

void tgfx_task_entry()
{
    touchgfx_taskEntry();
}

void tgfx_signal_dma_irq()
{
    touchgfx::HAL* hal = touchgfx::HAL::getInstance();
    if (hal != 0)
    {
        hal->signalDMAInterrupt();
    }
}
