#include "touchgfx_shim.h"

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
