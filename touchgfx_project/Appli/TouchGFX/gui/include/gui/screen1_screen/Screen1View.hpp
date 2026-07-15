#ifndef SCREEN1VIEW_HPP
#define SCREEN1VIEW_HPP

#include <gui_generated/screen1_screen/Screen1ViewBase.hpp>
#include <gui/screen1_screen/Screen1Presenter.hpp>

class Screen1View : public Screen1ViewBase
{
public:
    Screen1View();
    virtual ~Screen1View() {}
    virtual void setupScreen();
    virtual void tearDownScreen();

    // Board I/O bridged to the Rust firmware:
    //   - each tick: slider1_greenLED -> green LED (PO1) blink rate 0..100 Hz,
    //     toggleButton_redLED -> red LED (PG10), toggleButton_chromeart ->
    //     ChromART (DMA2D) hardware blitting, and the MCU load -> textArea3_CpuUse
    //   - the physical USER button (key 1) flips toggleButton_redLED
    virtual void handleTickEvent();
    virtual void handleKeyEvent(uint8_t key);

protected:
    int16_t lastGreenHz;
    bool lastRedOn;

    // ── Bouncing logos ("DVD logo" inside boxWithBorder1) ────────────────
    // 8x ferris_small (99x55) + 8x tgfx_logo (108x23), all plain Image
    // widgets drawn at native size -> every one is a ChromART blit.
    static const uint8_t NUM_LOGOS = 16;
    struct Mover
    {
        touchgfx::Drawable* d;
        int16_t vx;
        int16_t vy;
    };
    Mover movers[NUM_LOGOS];
    void bounce(Mover& m);

    // ── ChromART toggle + MCU-load read-out ──────────────────────────────
    uint8_t lastLoad;
    bool lastDmaOn;
    void updateCpuText();
};

#endif // SCREEN1VIEW_HPP
