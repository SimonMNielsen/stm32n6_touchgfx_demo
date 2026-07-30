/* RAM-only memory layout for STM32N657X0 on the STM32N6570-DK (TouchGFX demo).
 *
 * probe-rs downloads the ELF directly into AXISRAM and executes it.
 *
 * The executable is downloaded into the reset-visible AXISRAM window. The
 * board startup code enables all remaining SRAM banks before using them.
 *
 * Downloaded content is now about 224 KiB after removing unused images and
 * libstdc++. Keep the original proven code/RAM split for this board.
 */
MEMORY
{
  FLASH (rx)  : ORIGIN = 0x34000000, LENGTH = 1888K
  RAM   (rwx) : ORIGIN = 0x341D8000, LENGTH = 160K
}

/* Stack grows down from the top of the reset-enabled AXISRAM window. */
_stack_start = 0x34200000;
