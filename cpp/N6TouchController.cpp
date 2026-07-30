#include "N6TouchController.hpp"

#include "touchgfx_rust_callbacks.h"

bool N6TouchController::sampleTouch(int32_t& x, int32_t& y)
{
    int32_t rx = 0;
    int32_t ry = 0;
    if (rust_touch_sample(&rx, &ry))
    {
        x = rx;
        y = ry;
        return true;
    }
    return false;
}
