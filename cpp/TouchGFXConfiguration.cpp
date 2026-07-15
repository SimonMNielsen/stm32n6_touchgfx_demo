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
#include <touchgfx/hal/NoDMA.hpp>

// One-translation-unit instantiation of the RGB565 painters (was in
// TouchGFXGeneratedHAL.cpp).
#include <touchgfx/hal/PaintImpl.hpp>
#include <touchgfx/hal/PaintRGB565Impl.hpp>
#include <touchgfx/widgets/canvas/CWRVectorRenderer.hpp>

#include "N6TouchGFXHAL.hpp"
#include "N6TouchController.hpp"
#include "N6ButtonController.hpp"

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
static NoDMA dma;
static LCD16bpp display;
static VectorFontRendererImpl vectorFontRenderer;

static ApplicationFontProvider fontProvider;
static Texts texts;
static N6TouchGFXHAL hal(dma, display, tc, 800, 480);
static N6ButtonController buttonController;

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
}

void touchgfx_taskEntry()
{
    /*
     * Main event loop: HAL::taskEntry() loops forever on
     * OSWrappers::waitForVSync() (bridged to Rust) + backPorchExited().
     */
    hal.taskEntry();
}
