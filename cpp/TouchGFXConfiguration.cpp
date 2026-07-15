// TouchGFX framework wiring for the STM32N6570-DK Rust build.
//
// Adapted from the generated TouchGFXConfiguration.cpp of the original H750
// project (TouchGFX Generator 4.26.0), with the board-specific pieces
// replaced:
//   STM32DMA (Chrom-ART/DMA2D) -> NoDMA        (CPU rendering for bring-up)
//   STM32TouchController       -> N6TouchController (GT911 via Rust bridge)
//   TouchGFXHAL (H7 LTDC regs) -> N6TouchGFXHAL     (embassy LTDC via bridge)
//
// The VectorRenderer + Paint instantiations from TouchGFXGeneratedHAL.cpp
// live here too, since that file is not compiled in this build.

#include <texts/TypedTextDatabase.hpp>
#include <fonts/ApplicationFontProvider.hpp>
#include <gui/common/FrontendHeap.hpp>
#include <BitmapDatabase.hpp>
#include <touchgfx/VectorFontRendererImpl.hpp>
#include <platform/driver/lcd/LCD16bpp.hpp>
#include <CortexMMCUInstrumentation.hpp>

// One-translation-unit instantiation of the RGB565 painters (was in
// TouchGFXGeneratedHAL.cpp).
#include <touchgfx/hal/PaintImpl.hpp>
#include <touchgfx/hal/PaintRGB565Impl.hpp>
#include <touchgfx/widgets/canvas/CWRVectorRenderer.hpp>

#include "N6TouchGFXHAL.hpp"
#include "N6TouchController.hpp"
#include "N6ButtonController.hpp"
#include "N6ChromArtDMA.hpp"

extern "C" void touchgfx_init();
extern "C" void touchgfx_taskEntry();

using namespace touchgfx;

namespace touchgfx
{
VectorRenderer* VectorRenderer::getInstance()
{
    static CWRVectorRendererRGB565 renderer;

    return &renderer;
}
} // namespace touchgfx

static N6TouchController tc;
static N6ChromArtDMA dma; // ChromART (DMA2D) hardware blitter
static LCD16bpp display;
static VectorFontRendererImpl vectorFontRenderer;

static ApplicationFontProvider fontProvider;
static Texts texts;
static N6TouchGFXHAL hal(dma, display, tc, 800, 480);
static N6ButtonController buttonController;

// MCU-load measurement (DWT cycle counter; the counter itself is enabled in
// Rust main()). Screen1View reads it via tgfx_get_mcu_load_pct() and renders
// it into the GUI's textArea3_CpuUse wildcard.
static CortexMMCUInstrumentation instrumentation;

void touchgfx_init()
{
    Bitmap::registerBitmapDatabase(BitmapDatabase::getInstance(), BitmapDatabase::getInstanceSize());
    TypedText::registerTexts(&texts);
    Texts::setLanguage(0);

    display.setVectorFontRenderer(&vectorFontRenderer);

    FontManager::setFontProvider(&fontProvider);

    FrontendHeap& heap = FrontendHeap::getInstance();
    /*
     * we need to obtain the reference above to initialize the frontend heap.
     */
    (void)heap;

    /*
     * Initialize TouchGFX
     */
    hal.initialize();

    // USER button (PC13) → key events (sampled once per tick via the bridge).
    hal.setButtonController(&buttonController);

    // MCU-load measurement (drives the on-screen "CPU xx%").
    instrumentation.init();
    hal.setMCUInstrumentation(&instrumentation);
    hal.enableMCULoadCalculation(true);
}

// Runtime ChromART on/off from the GUI toggle. When disabled TouchGFX's
// getBlitCaps() returns 0 and every blit falls back to software rendering —
// which is exactly what makes the CPU% difference visible.
void tgfx_set_dma_enabled(bool enabled)
{
    hal.enableDMAAcceleration(enabled);
}

unsigned char tgfx_get_mcu_load_pct()
{
    return hal.getMCULoadPct();
}

void touchgfx_taskEntry()
{
    /*
     * Main event loop: HAL::taskEntry() loops forever on
     * OSWrappers::waitForVSync() (bridged to Rust) + backPorchExited().
     */
    hal.taskEntry();
}
