// touchgfx::GPIO — performance-measurement signal hooks.
//
// The original H750 project drove board pins here (VSYNC_FREQ, RENDER_TIME,
// FRAME_RATE, MCU_ACTIVE) for scope-based profiling. Not wired on the N6
// build — no-ops. Bridge these into Rust later if profiling is wanted.

#include <touchgfx/hal/GPIO.hpp>

using namespace touchgfx;

void GPIO::init() {}

void GPIO::set(GPIO_ID id)
{
    (void)id;
}

void GPIO::clear(GPIO_ID id)
{
    (void)id;
}

void GPIO::toggle(GPIO_ID id)
{
    (void)id;
}

bool GPIO::get(GPIO_ID id)
{
    (void)id;
    return false;
}
