//! Application build: memory map, board-integration C++, and app-specific
//! link args. The reusable half of a TouchGFX build (Designer-project
//! includes/sources, framework core, Arm runtime, constructor linker
//! fragment) lives in `touchgfx-rs::build_helpers`.

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let paths_file = manifest.join("paths.txt");
    println!("cargo:rerun-if-changed={}", paths_file.display());
    let paths = touchgfx_rs::build_helpers::TouchGfxPaths::from_file(&paths_file);

    // App memory map: emit memory.x from memory_ram.x and expose OUT_DIR.
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    File::create(out.join("memory.x"))
        .expect("create memory.x")
        .write_all(include_bytes!("memory_ram.x"))
        .expect("write memory.x");
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory_ram.x");

    // Reusable TouchGFX-side linker fragment (touchgfx_ctors.x + -T).
    touchgfx_rs::build_helpers::emit_linker_script(&out);

    // App-owned C++: TouchGFXConfiguration.cpp is the one file that encodes
    // application policy (panel size 800x480, RGB565 pixel format, DMA
    // driver choice, MCU-load instrumentation). Every other adapter class
    // lives in the touchgfx-rs crate and is swept in automatically by
    // compile_designer_project via its own cpp/ directory.
    println!("cargo:rerun-if-changed=cpp");
    println!("cargo:rerun-if-changed=src/bridge.rs");

    let board_cpp = manifest.join("cpp");
    let board_sources = [board_cpp.join("TouchGFXConfiguration.cpp")];

    let build = touchgfx_rs::build_helpers::compile_designer_project(
        &paths,
        [board_cpp.as_path()],
        board_sources.iter().map(PathBuf::as_path),
    );

    // Link the prebuilt TouchGFX core archive and the Arm GNU Toolchain
    // runtime (both selected by the compiler `build` was configured with).
    touchgfx_rs::build_helpers::link_core_library(&paths.core_library);
    touchgfx_rs::build_helpers::link_arm_runtime(&build);

    // App-specific link args:
    //   * `end=0x90400000` — newlib heap starts in initialised PSRAM.
    //   * `__exidx_*=0`    — stub out ARM exception tables (no unwinding).
    //   * `--nmagic`       — no page-align, packs sections tight.
    //   * `-Tlink.x`       — cortex-m-rt entry.
    //   * `-Tdefmt.x`      — defmt logging tables.
    println!("cargo:rustc-link-arg-bins=--defsym=end=0x90400000");
    println!("cargo:rustc-link-arg-bins=--defsym=__exidx_start=0");
    println!("cargo:rustc-link-arg-bins=--defsym=__exidx_end=0");
    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
