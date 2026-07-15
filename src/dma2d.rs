//! ChromART (DMA2D) blit engine for TouchGFX — pure embassy PAC, no ST HAL.
//!
//! The C++ `N6ChromArtDMA` (cpp/N6ChromArtDMA.cpp) implements TouchGFX's
//! `DMA_Interface` and hands each blit here as already-decoded register
//! values; this module owns the actual hardware: clock, register programming,
//! NVIC, and the completion interrupt that advances TouchGFX's blit queue.
//!
//! The register sequences mirror ST's reference `STM32DMA.cpp` exactly (it is
//! itself almost pure `WRITE_REG(DMA2D->...)`), just expressed through the
//! metapac instead of the HAL.
//!
//! Note on the N6: DMA2D is the *old* ChromART blitter (rect fill/copy/blend),
//! not the NeoChrom GPU2D (which has no embassy driver). It can't scale or
//! rotate, so TouchGFX still falls back to the CPU for those.

use embassy_stm32::interrupt;
use embassy_stm32::interrupt::InterruptExt;
use embassy_stm32::pac::dma2d::regs;
use embassy_stm32::pac::{DMA2D, RCC};

// ── CR bits (RM0486 DMA2D_CR) ────────────────────────────────────────────────
const CR_START: u32 = 1 << 0;
const CR_TEIE: u32 = 1 << 8; // transfer error IRQ
const CR_TCIE: u32 = 1 << 9; // transfer complete IRQ
const CR_CEIE: u32 = 1 << 13; // configuration error IRQ
/// The IRQ set ST enables on every blit (TC drives the queue; TE/CE surface faults).
const CR_IRQS: u32 = CR_TCIE | CR_TEIE | CR_CEIE;

/// CR.MODE (bits 17:16)
pub const MODE_M2M: u32 = 0 << 16; // copy, no conversion
pub const MODE_M2M_PFC: u32 = 1 << 16; // copy + pixel-format convert
pub const MODE_M2M_BLEND: u32 = 2 << 16; // fg over bg blend
const MODE_R2M: u32 = 3 << 16; // register (colour) to memory = solid fill

/// All IFCR clear bits (CTEIF/CTCIF/CTWIF/CAECIF/CCTCIF/CCEIF).
const IFCR_CLEAR_ALL: u32 = 0x3F;

/// Bring up ChromART: clock + a clean interrupt state. The NVIC line stays
/// masked until TouchGFX calls `enableInterrupts()` (see N6TouchGFXHAL).
pub fn init() {
    RCC.ahb5enr().modify(|w| w.set_dma2den(true));
    cortex_m::asm::dsb();

    // Make sure nothing is running and no stale flags are latched.
    DMA2D.cr().write_value(regs::Cr(0));
    DMA2D.ifcr().write_value(regs::Ifcr(IFCR_CLEAR_ALL));
}

/// Priority for the blit-complete IRQ. It runs TouchGFX's queue handling, so
/// keep it below the SPI6 InterruptExecutor (P7) — i.e. a *higher* numeric
/// priority value, which on Cortex-M means lower urgency.
pub fn configure_irq() {
    interrupt::DMA2D.set_priority(interrupt::Priority::P8);
}

/// Unmask the blit-complete IRQ (TouchGFX `HAL::enableInterrupts`).
pub fn enable_irq() {
    unsafe { interrupt::DMA2D.enable() };
}

/// Mask the blit-complete IRQ. TouchGFX brackets its blit-queue updates with
/// this, so the queue is never mutated while the ISR is popping from it.
pub fn disable_irq() {
    interrupt::DMA2D.disable();
}

/// Solid colour fill (`BLIT_OP_FILL`) — register-to-memory.
///
/// `out_pfccr` is the decoded output colour mode, `dst_off` the destination
/// line offset in pixels (`dstLoopStride - nSteps`).
pub fn fill(dst: u32, out_pfccr: u32, color: u32, dst_off: u16, n_steps: u16, n_loops: u16) {
    DMA2D.opfccr().write_value(regs::Opfccr(out_pfccr));
    DMA2D.nlr().write_value(regs::Nlr(nlr(n_steps, n_loops)));
    DMA2D.omar().write_value(regs::Omar(dst));
    DMA2D.oor().write_value(regs::Oor(dst_off as u32));
    // FGPFCCR mirrors the output format with no alpha modification, matching
    // ST's fill path; FGOR must be 0 for R2M.
    DMA2D.fgpfccr().write_value(regs::Fgpfccr(out_pfccr));
    DMA2D.fgor().write_value(regs::Fgor(0));
    DMA2D.ocolr().write_value(regs::Ocolr(color));
    start(MODE_R2M);
}

/// Fill blended with a constant alpha (`BLIT_OP_FILL_WITH_ALPHA`): an A8
/// foreground of `color` at `alpha`, blended over the destination itself.
pub fn fill_alpha(
    dst: u32,
    out_pfccr: u32,
    fg_pfccr: u32,
    bg_pfccr: u32,
    color: u32,
    dst_off: u16,
    n_steps: u16,
    n_loops: u16,
) {
    DMA2D.opfccr().write_value(regs::Opfccr(out_pfccr));
    DMA2D.nlr().write_value(regs::Nlr(nlr(n_steps, n_loops)));
    DMA2D.omar().write_value(regs::Omar(dst));
    DMA2D.oor().write_value(regs::Oor(dst_off as u32));
    DMA2D.bgor().write_value(regs::Bgor(dst_off as u32));
    DMA2D.fgor().write_value(regs::Fgor(dst_off as u32));
    DMA2D.bgpfccr().write_value(regs::Bgpfccr(bg_pfccr));
    DMA2D.fgpfccr().write_value(regs::Fgpfccr(fg_pfccr));
    DMA2D.fgcolr().write_value(regs::Fgcolr(color));
    DMA2D.bgmar().write_value(regs::Bgmar(dst));
    DMA2D.fgmar().write_value(regs::Fgmar(dst));
    start(MODE_M2M_BLEND);
}

/// Image blit. `mode` is one of [`MODE_M2M`], [`MODE_M2M_PFC`] or
/// [`MODE_M2M_BLEND`]; the C++ side decodes the TouchGFX blit op into that
/// plus the three PFCCR values. `fg_colr` only matters for the A4/A8 text
/// paths (it carries the text colour).
#[allow(clippy::too_many_arguments)]
pub fn copy(
    mode: u32,
    src: u32,
    dst: u32,
    fg_pfccr: u32,
    bg_pfccr: u32,
    out_pfccr: u32,
    fg_colr: u32,
    src_off: u16,
    dst_off: u16,
    n_steps: u16,
    n_loops: u16,
) {
    DMA2D.oor().write_value(regs::Oor(dst_off as u32));
    DMA2D.bgor().write_value(regs::Bgor(dst_off as u32));
    DMA2D.fgor().write_value(regs::Fgor(src_off as u32));
    DMA2D.opfccr().write_value(regs::Opfccr(out_pfccr));
    DMA2D.nlr().write_value(regs::Nlr(nlr(n_steps, n_loops)));
    DMA2D.omar().write_value(regs::Omar(dst));
    DMA2D.fgmar().write_value(regs::Fgmar(src));
    DMA2D.fgpfccr().write_value(regs::Fgpfccr(fg_pfccr));
    DMA2D.fgcolr().write_value(regs::Fgcolr(fg_colr));

    if mode == MODE_M2M_BLEND {
        // Blending reads the destination as the background layer.
        DMA2D.bgpfccr().write_value(regs::Bgpfccr(bg_pfccr));
        DMA2D.bgmar().write_value(regs::Bgmar(dst));
    }
    start(mode);
}

/// NLR: PL (pixels per line) in 31:16, NL (number of lines) in 15:0.
#[inline]
fn nlr(n_steps: u16, n_loops: u16) -> u32 {
    (n_loops as u32) | ((n_steps as u32) << 16)
}

/// Arm the transfer: mode + completion/error IRQs + START.
#[inline]
fn start(mode: u32) {
    cortex_m::asm::dsb(); // all descriptor writes visible before the engine reads them
    DMA2D.cr().write_value(regs::Cr(mode | CR_IRQS | CR_START));
}

/// Blit-complete (or error) interrupt: clear the flags and let TouchGFX pop
/// the next queued blit. `signalDMAInterrupt` runs the framework's queue
/// handling in this ISR — the same arrangement ST's DMA2D callback uses.
#[interrupt]
unsafe fn DMA2D() {
    let isr = DMA2D.isr().read();
    DMA2D.ifcr().write_value(regs::Ifcr(IFCR_CLEAR_ALL));

    if isr.teif() || isr.ceif() {
        defmt::warn!(
            "DMA2D error: teif={=bool} ceif={=bool}",
            isr.teif(),
            isr.ceif()
        );
    }

    crate::bridge::ffi::tgfx_signal_dma_irq();
}
