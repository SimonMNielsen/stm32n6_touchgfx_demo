#include "N6ButtonController.hpp"

#include "touchgfx_rust_callbacks.h"

bool N6ButtonController::sample(uint8_t& key)
{
    uint8_t k = 0;
    if (rust_button_sample(&k))
    {
        key = k;
        return true;
    }
    return false;
}
