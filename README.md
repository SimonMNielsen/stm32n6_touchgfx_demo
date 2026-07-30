# STM32N6 TouchGFX Rust demo

Application-specific STM32N6570-DK demo using the reusable `touchgfx-rs` crate
from `../internal_crates/touchgfx-rs`.

This project now contains only:

- board startup, display/touch/button/LED setup, and memory layout;
- framebuffer/vsync/input callbacks for this board;
- the board-specific TouchGFX HAL/controllers/configuration;
- the TouchGFX Designer project, generated GUI, assets, and demo behavior.

Generic OS wrappers, C++ runtime glue, entry-point shims, constructor/asset
linker sections, ChromART C++, DMA2D Rust code, and build helpers live in
`touchgfx-rs`.

## Configure TouchGFX paths

Edit `paths.txt` in this project root. It contains the only two external
TouchGFX locations used by the build:

- `designer_project`: directory containing `gui`, `generated`, `target`, and
   `application.config`;
- `core_library`: exact prebuilt `libtouchgfx-*.a` archive matching this target.

Paths can be absolute or relative to `paths.txt`; optional double quotes allow
spaces. The framework headers are found automatically from an ancestor of the
configured core archive.

## Generate vendor sources first

The default relative Designer project currently does not contain its generated
`TouchGFX/generated` tree or `Middlewares/ST/touchgfx` framework/core files.
Before Cargo can compile the final firmware:

1. open `touchgfx_project/Appli/TouchGFX/touchgfx_2_rust_demo.touchgfx` in
   TouchGFX Designer and generate code;
2. ensure the selected TouchGFX installation/project supplies the framework
   headers and Cortex-M55 hard-float core library under
   `touchgfx_project/Appli/Middlewares/ST/touchgfx`;
3. run `cargo build --bin touchgfx_demo`.

The Rust runtime and host build-support portions of `touchgfx-rs` can be
checked independently even when those generated/vendor files are absent.
