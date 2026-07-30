#include <gui/screen1_screen/Screen1View.hpp>

#include <touchgfx/Unicode.hpp>

// Application-specific Rust firmware callbacks (LED control).
#include "AppCallbacks.h"
// Firmware glue: ChromART on/off + MCU load.
#include "touchgfx_shim.h"

Screen1View::Screen1View()
    : lastGreenHz(-1), lastRedOn(false), lastLoad(255), lastDmaOn(true)
{
}

void Screen1View::setupScreen()
{
    Screen1ViewBase::setupScreen();

    // All sixteen logos bounce inside boxWithBorder1. Distinct velocity vectors
    // so they drift apart instead of moving as a block; they pass over each
    // other freely (no collision handling).
    const int16_t vel[NUM_LOGOS][2] = {
        {  3,  2 }, { -2,  3 }, {  2, -3 }, { -3, -2 }, {  4,  1 }, { -1,  4 },
        {  2,  4 }, { -4,  2 }, {  3, -1 }, { -1, -3 }, {  1,  3 }, { -3,  1 },
        {  4, -2 }, { -2, -4 }, {  1, -4 }, { -4,  3 }
    };
    // Every logo is a plain Image at native size (ferris_small 99x55,
    // tgfx_logo 108x23), so each redraw is a ChromART blit.
    touchgfx::Drawable* logos[NUM_LOGOS] = {
        &rust_logo,   &rust_logo_1, &rust_logo_2, &rust_logo_3,
        &rust_logo_4, &rust_logo_7, &rust_logo_8, &rust_logo_6_1,
        &tgfx_logo,   &tgfx_logo_1, &tgfx_logo_2, &tgfx_logo_3,
        &tgfx_logo_4, &tgfx_logo_5, &tgfx_logo_6, &tgfx_logo_7
    };
    for (uint8_t i = 0; i < NUM_LOGOS; i++)
    {
        movers[i].d = logos[i];
        movers[i].vx = vel[i][0];
        movers[i].vy = vel[i][1];
    }

    // ChromART starts enabled; keep the toggle in sync with the firmware.
    toggleButton_chromeart.forceState(true);
    toggleButton_chromeart.invalidate();
    tgfx_set_dma_enabled(true);
    lastDmaOn = true;

    lastLoad = 0;
    updateCpuText();
}

void Screen1View::tearDownScreen()
{
    Screen1ViewBase::tearDownScreen();
}

// Move `m` by its velocity, reflecting off the inner edge of boxWithBorder1.
// moveTo() invalidates the vacated and the new area, so only the logo rects
// are redrawn — cheap even on the non-cacheable PSRAM framebuffer.
void Screen1View::bounce(Mover& m)
{
    const int16_t border = 5; // boxWithBorder1.setBorderSize(5)
    const int16_t left = boxWithBorder1.getX() + border;
    const int16_t top = boxWithBorder1.getY() + border;
    const int16_t right = boxWithBorder1.getX() + boxWithBorder1.getWidth() - border;
    const int16_t bottom = boxWithBorder1.getY() + boxWithBorder1.getHeight() - border;

    int16_t nx = m.d->getX() + m.vx;
    int16_t ny = m.d->getY() + m.vy;
    const int16_t maxX = right - m.d->getWidth();
    const int16_t maxY = bottom - m.d->getHeight();

    if (nx <= left)      { nx = left; m.vx = -m.vx; }
    else if (nx >= maxX) { nx = maxX; m.vx = -m.vx; }
    if (ny <= top)       { ny = top; m.vy = -m.vy; }
    else if (ny >= maxY) { ny = maxY; m.vy = -m.vy; }

    m.d->moveTo(nx, ny);
}

// Fill the "<>%" wildcard with the current MCU load. Unicode::snprintf is
// TouchGFX's own formatter (no newlib printf), which matters here: FLASH is
// nearly full.
void Screen1View::updateCpuText()
{
    const uint8_t load = lastLoad > 100 ? 100 : lastLoad;
    touchgfx::Unicode::snprintf(textArea3_CpuUseBuffer, TEXTAREA3_CPUUSE_SIZE, "%u",
                                static_cast<unsigned int>(load));
    textArea3_CpuUse.invalidate();
}

void Screen1View::handleTickEvent()
{
    Screen1ViewBase::handleTickEvent();

    for (uint8_t i = 0; i < NUM_LOGOS; i++)
    {
        bounce(movers[i]);
    }

    // Green LED blink rate follows the slider (0..100 Hz; 0 = off).
    const int16_t greenHz = slider1_greenLED.getValue();
    if (greenHz != lastGreenHz)
    {
        lastGreenHz = greenHz;
        rust_set_green_hz(static_cast<uint8_t>(greenHz));
    }

    // Red LED follows the toggle button.
    const bool redOn = toggleButton_redLED.getState();
    if (redOn != lastRedOn)
    {
        lastRedOn = redOn;
        rust_set_red(redOn);
    }

    // ChromART hardware blitting on/off. With it off TouchGFX's getBlitCaps()
    // returns 0 and every blit is software-rendered — watch the CPU% change.
    const bool dmaOn = toggleButton_chromeart.getState();
    if (dmaOn != lastDmaOn)
    {
        lastDmaOn = dmaOn;
        tgfx_set_dma_enabled(dmaOn);
    }

    const uint8_t load = tgfx_get_mcu_load_pct();
    if (load != lastLoad)
    {
        lastLoad = load;
        updateCpuText();
    }
}

void Screen1View::handleKeyEvent(uint8_t key)
{
    if (key == 1)
    {
        // Physical USER button flips the on-screen red-LED toggle; the change
        // reaches the LED through handleTickEvent above.
        toggleButton_redLED.forceState(!toggleButton_redLED.getState());
        toggleButton_redLED.invalidate();
    }
}
