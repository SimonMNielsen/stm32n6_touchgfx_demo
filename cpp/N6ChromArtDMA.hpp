// TouchGFX DMA_Interface backed by the N6's ChromART (DMA2D) blitter.
//
// This replaces ST's STM32DMA (which is written against the ST HAL). All the
// hardware lives in Rust (src/dma2d.rs) behind the cxx bridge; this class only
// decodes TouchGFX BlitOps into DMA2D register values and enqueues them.
//
// ChromART is the classic 2D blitter: rectangle fill / copy / format-convert /
// alpha-blend. It cannot scale or rotate (that's the NeoChrom GPU2D, which has
// no embassy driver), so TouchGFX keeps rendering those on the CPU.
#pragma once

#include <touchgfx/Bitmap.hpp>
#include <touchgfx/hal/DMA.hpp>

class N6ChromArtDMA : public touchgfx::DMA_Interface
{
    typedef touchgfx::DMA_Interface Base;

public:
    N6ChromArtDMA();
    virtual ~N6ChromArtDMA();

    virtual touchgfx::BlitOperations getBlitCaps();

    /// Called from the DMA2D ISR (via the Rust bridge) when a blit completes.
    virtual void signalDMAInterrupt();

    virtual void initialize();

protected:
    virtual void setupDataCopy(const touchgfx::BlitOp& blitOp);
    virtual void setupDataFill(const touchgfx::BlitOp& blitOp);

private:
    // Declaration order matters: queueStorage must be constructed before
    // dmaQueue, which the base DMA_Interface ctor takes a reference to.
    touchgfx::BlitOp queueStorage[96];
    touchgfx::LockFreeDMA_Queue dmaQueue;

    static uint32_t inputFormat(touchgfx::Bitmap::BitmapFormat format);
    static uint32_t outputFormat(touchgfx::Bitmap::BitmapFormat format);
};
