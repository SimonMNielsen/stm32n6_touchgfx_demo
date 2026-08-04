// TouchGFX framework wiring for the STM32N6570-DK Rust build.
//
// This is the one place where application/board policy is encoded:
//   * Panel resolution              : 800x480
//   * Pixel format                  : RGB565 (LCD16bpp + CWRVectorRendererRGB565)
//   * DMA driver                    : TouchGfxChromArtDMA (STM32 DMA2D)
//   * MCU-load instrumentation      : CortexMMCUInstrumentation
//
// Everything else (touch/button controllers, HAL, GPIO hooks, the Rust
// callback ABI) is generic and lives in the touchgfx-rs integration crate.

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

#include "RustBridgedTouchGFXHAL.hpp"
#include "RustBridgedTouchController.hpp"
#include "RustBridgedButtonController.hpp"
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

static RustBridgedTouchController tc;
static TouchGfxChromArtDMA dma; // ChromART (DMA2D) hardware blitter
static LCD16bpp display;
static VectorFontRendererImpl vectorFontRenderer;

static ApplicationFontProvider fontProvider;
static Texts texts;
static RustBridgedTouchGFXHAL hal(dma, display, tc, 800, 480);
static RustBridgedButtonController buttonController;

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
