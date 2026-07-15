/* RAM-only memory layout for STM32N657X0 on the STM32N6570-DK (TouchGFX demo).
 *
 * probe-rs downloads the ELF directly into AXISRAM and executes it.
 *
 * CRITICAL: only AXISRAM1-4 (0x3400_0000 .. 0x3420_0000, 2 MB) is enabled at
 * reset / during the probe-rs download. AXISRAM5-6 (0x3420_0000+) is gated off
 * until main() sets RCC.MEMENR, so NOTHING that is downloaded, nor the stack
 * or .data/.bss, may live above 0x3420_0000 — doing so wedges the AXI bus in
 * the reset handler before a single log line (silent SwdApFault). AXISRAM5-6
 * is used only at runtime for... nothing here; the 800x480 framebuffers live
 * in PSRAM (see src/bridge.rs), which keeps this whole image inside 2 MB.
 *
 * Budget (2 MB): downloaded content (.text + .rodata + ~1.7 MB of
 * .tgfx_assets image/font data) ≈ 1.89 MB in FLASH; the rest is RAM for
 * .data/.bss + heap + stack. FLASH is the tight one — adding glyphs or
 * images pushes it over, and the only slack is this split (RAM only really
 * needs ~12 KB + stack; the framebuffers live in PSRAM).
 */
MEMORY
{
  FLASH (rx)  : ORIGIN = 0x34000000, LENGTH = 1888K
  RAM   (rwx) : ORIGIN = 0x341D8000, LENGTH = 160K
}

/* Stack grows down from the top of the reset-enabled AXISRAM window. */
_stack_start = 0x34200000;
