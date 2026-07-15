#include "N6ButtonController.hpp"

#include "stm32n6_touchgfx_demo/src/bridge.rs.h"

bool N6ButtonController::sample(uint8_t& key)
{
    uint8_t k = 0;
    if (rust_button_sample(k))
    {
        key = k;
        return true;
    }
    return false;
}
