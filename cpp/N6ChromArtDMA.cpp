#include "N6ChromArtDMA.hpp"

#include <touchgfx/hal/HAL.hpp>

#include "stm32n6_touchgfx_demo/src/bridge.rs.h"

using namespace touchgfx;

namespace
{
// ── DMA2D field encodings (RM0486; same values as ST's HAL macros) ──────────
// FGPFCCR/BGPFCCR colour mode (bits 3:0)
const uint32_t INPUT_ARGB8888 = 0x0;
const uint32_t INPUT_RGB888 = 0x1;
const uint32_t INPUT_RGB565 = 0x2;
const uint32_t INPUT_L8 = 0x5;
const uint32_t INPUT_A8 = 0x9;
const uint32_t INPUT_A4 = 0xA;

// OPFCCR colour mode (bits 2:0)
const uint32_t OUTPUT_ARGB8888 = 0x0;
const uint32_t OUTPUT_RGB888 = 0x1;
const uint32_t OUTPUT_RGB565 = 0x2;

// xPFCCR alpha mode (bits 17:16) and ALPHA (bits 31:24)
const uint32_t AM_POS = 16;
const uint32_t NO_MODIF_ALPHA = 0x0;
const uint32_t REPLACE_ALPHA = 0x1;
const uint32_t COMBINE_ALPHA = 0x2;
const uint32_t ALPHA_POS = 24;

// CR.MODE selector passed to the Rust side.
const uint8_t MODE_M2M = 0;
const uint8_t MODE_M2M_PFC = 1;
const uint8_t MODE_M2M_BLEND = 2;
} // namespace

uint32_t N6ChromArtDMA::inputFormat(Bitmap::BitmapFormat format)
{
    switch (format)
    {
    case Bitmap::ARGB8888:
        return INPUT_ARGB8888;
    case Bitmap::RGB888:
        return INPUT_RGB888;
    case Bitmap::RGB565:
        return INPUT_RGB565;
    case Bitmap::ARGB2222: /* fall through */
    case Bitmap::ABGR2222: /* fall through */
    case Bitmap::RGBA2222: /* fall through */
    case Bitmap::BGRA2222: /* fall through */
    case Bitmap::L8:
        return INPUT_L8;
    default:
        // Unsupported by ChromART — getBlitCaps() never advertises these, so
        // TouchGFX renders them on the CPU and we should not get here.
        return INPUT_ARGB8888;
    }
}

uint32_t N6ChromArtDMA::outputFormat(Bitmap::BitmapFormat format)
{
    switch (format)
    {
    case Bitmap::ARGB8888:
        return OUTPUT_ARGB8888;
    case Bitmap::RGB888:
        return OUTPUT_RGB888;
    case Bitmap::RGB565:
        return OUTPUT_RGB565;
    default:
        return OUTPUT_RGB565;
    }
}

N6ChromArtDMA::N6ChromArtDMA()
    : Base(dmaQueue), dmaQueue(queueStorage, sizeof(queueStorage) / sizeof(queueStorage[0]))
{
}

N6ChromArtDMA::~N6ChromArtDMA()
{
    rust_dma2d_disable_irq();
}

void N6ChromArtDMA::initialize()
{
    rust_dma2d_configure_irq();
}

BlitOperations N6ChromArtDMA::getBlitCaps()
{
    // L8 is deliberately omitted: it needs a CLUT load + poll, and this app's
    // assets are RGB565/ARGB8888 (l8_compression is off in application.config).
    return static_cast<BlitOperations>(BLIT_OP_FILL
                                       | BLIT_OP_FILL_WITH_ALPHA
                                       | BLIT_OP_COPY
                                       | BLIT_OP_COPY_WITH_ALPHA
                                       | BLIT_OP_COPY_ARGB8888
                                       | BLIT_OP_COPY_ARGB8888_WITH_ALPHA
                                       | BLIT_OP_COPY_A4
                                       | BLIT_OP_COPY_A8);
}

void N6ChromArtDMA::signalDMAInterrupt()
{
    executeCompleted();
}

void N6ChromArtDMA::setupDataFill(const BlitOp& blitOp)
{
    const uint32_t outMode = outputFormat(static_cast<Bitmap::BitmapFormat>(blitOp.dstFormat));
    const uint16_t dstOff = blitOp.dstLoopStride - blitOp.nSteps;

    if (blitOp.operation == BLIT_OP_FILL_WITH_ALPHA)
    {
        // A8 foreground of `color` at `alpha`, blended over the destination.
        const uint32_t fgPfccr =
            INPUT_A8 | (REPLACE_ALPHA << AM_POS) | (static_cast<uint32_t>(blitOp.alpha) << ALPHA_POS);
        const uint32_t bgPfccr = outMode | (NO_MODIF_ALPHA << AM_POS);
        rust_dma2d_fill_alpha((uint32_t)blitOp.pDst, outMode, fgPfccr, bgPfccr,
                              (uint32_t)blitOp.color, dstOff, blitOp.nSteps, blitOp.nLoops);
    }
    else
    {
        rust_dma2d_fill((uint32_t)blitOp.pDst, outMode, (uint32_t)blitOp.color, dstOff,
                        blitOp.nSteps, blitOp.nLoops);
    }
}

void N6ChromArtDMA::setupDataCopy(const BlitOp& blitOp)
{
    const uint32_t fgMode = inputFormat(static_cast<Bitmap::BitmapFormat>(blitOp.srcFormat));
    const uint32_t bgMode = inputFormat(static_cast<Bitmap::BitmapFormat>(blitOp.dstFormat));
    const uint32_t outMode = outputFormat(static_cast<Bitmap::BitmapFormat>(blitOp.dstFormat));
    const uint32_t alpha = static_cast<uint32_t>(blitOp.alpha) << ALPHA_POS;

    const uint16_t srcOff = blitOp.srcLoopStride - blitOp.nSteps;
    const uint16_t dstOff = blitOp.dstLoopStride - blitOp.nSteps;
    const uint32_t bgPfccr = bgMode | (NO_MODIF_ALPHA << AM_POS);

    uint32_t fgPfccr;
    uint32_t fgColr = 0;
    uint8_t mode;

    switch (blitOp.operation)
    {
    case BLIT_OP_COPY_A4:
        // 4bpp glyph: alpha-only source tinted with the text colour.
        fgPfccr = INPUT_A4 | (COMBINE_ALPHA << AM_POS) | alpha;
        fgColr = (uint32_t)blitOp.color;
        mode = MODE_M2M_BLEND;
        break;

    case BLIT_OP_COPY_A8:
        fgPfccr = INPUT_A8 | (COMBINE_ALPHA << AM_POS) | alpha;
        fgColr = (uint32_t)blitOp.color;
        mode = MODE_M2M_BLEND;
        break;

    case BLIT_OP_COPY_WITH_ALPHA:
    case BLIT_OP_COPY_ARGB8888:
    case BLIT_OP_COPY_ARGB8888_WITH_ALPHA:
        fgPfccr = fgMode | (COMBINE_ALPHA << AM_POS) | alpha;
        mode = MODE_M2M_BLEND;
        break;

    default: // BLIT_OP_COPY
        fgPfccr = fgMode | (COMBINE_ALPHA << AM_POS) | alpha;
        // Straight copy when the formats already match, otherwise let the
        // pixel-format converter do the work.
        mode = (blitOp.srcFormat != blitOp.dstFormat) ? MODE_M2M_PFC : MODE_M2M;
        break;
    }

    rust_dma2d_copy(mode, (uint32_t)blitOp.pSrc, (uint32_t)blitOp.pDst, fgPfccr, bgPfccr, outMode,
                    fgColr, srcOff, dstOff, blitOp.nSteps, blitOp.nLoops);
}
