/* TouchGFX-specific output sections, inserted into the cortex-m-rt layout.
 *
 * 1. .init_array — C++ static constructors (cortex-m-rt's link.x doesn't
 *    collect these; src/main.rs walks the table and calls each one).
 * 2. Asset sections — the TouchGFX image/font/text data is emitted with
 *    __attribute__((section("ExtFlashSection"))) etc. by the gcc build. On
 *    the production N6 target these go to external NOR; for this RAM app we
 *    fold them into FLASH (AXISRAM) so the whole image is a single probe-rs
 *    download. ~1.7 MB total — FLASH is sized for it in memory_ram.x.
 */
SECTIONS
{
  .init_array : ALIGN(4)
  {
    __init_array_start = .;
    KEEP(*(SORT_BY_INIT_PRIORITY(.init_array.*)));
    KEEP(*(.init_array));
    __init_array_end = .;
  } > FLASH

  .tgfx_assets : ALIGN(32)
  {
    KEEP(*(ExtFlashSection ExtFlashSection.*));
    KEEP(*(FontFlashSection FontFlashSection.*));
    KEEP(*(FontSearchFlashSection FontSearchFlashSection.*));
    KEEP(*(TextFlashSection TextFlashSection.*));
  } > FLASH
} INSERT AFTER .rodata;
