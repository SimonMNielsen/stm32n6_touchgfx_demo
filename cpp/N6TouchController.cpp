#include "N6TouchController.hpp"

#include "stm32n6_touchgfx_demo/src/bridge.rs.h"

bool N6TouchController::sampleTouch(int32_t& x, int32_t& y)
{
    int32_t rx = 0;
    int32_t ry = 0;
    if (rust_touch_sample(rx, ry))
    {
        x = rx;
        y = ry;
        return true;
    }
    return false;
}
