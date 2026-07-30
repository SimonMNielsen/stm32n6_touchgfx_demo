//! Application build: memory map, board-specific C++, and generated GUI.
//! Reusable TouchGFX glue/build policy comes from touchgfx-rs.

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let paths_file = manifest.join("paths.txt");
    println!("cargo:rerun-if-changed={}", paths_file.display());
    let paths = touchgfx_rs::build::TouchGfxPaths::from_file(&paths_file);
    let tg = &paths.designer_project;
    let framework = paths.framework_root();

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    File::create(out.join("memory.x"))
        .expect("create memory.x")
        .write_all(include_bytes!("memory_ram.x"))
        .expect("write memory.x");
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory_ram.x");

    touchgfx_rs::build::emit_linker_script(&out);

    println!("cargo:rerun-if-changed=cpp");
    println!("cargo:rerun-if-changed=src/bridge.rs");
    println!("cargo:rerun-if-changed={}", tg.join("gui/src").display());
    println!("cargo:rerun-if-changed={}", tg.join("gui/include").display());
    println!("cargo:rerun-if-changed={}", tg.join("generated").display());

    let mut build = cc::Build::new();
    touchgfx_rs::build::configure_cpp(&mut build);
    build
        .include(manifest.join("cpp"))
        .include(framework.join("framework/include"))
        .include(tg.join("target"))
        .include(tg.join("gui/include"))
        .include(tg.join("generated/gui_generated/include"))
        .include(tg.join("generated/fonts/include"))
        .include(tg.join("generated/images/include"))
        .include(tg.join("generated/texts/include"));

    // Board integration and application assembly only.
    for source in [
        "N6ButtonController.cpp",
        "N6TouchController.cpp",
        "N6TouchGFXHAL.cpp",
        "TouchGFXConfiguration.cpp",
        "TouchGFXGPIO.cpp",
    ] {
        build.file(manifest.join("cpp").join(source));
    }

    build.file(tg.join("target/CortexMMCUInstrumentation.cpp"));
    for relative in [
        "gui/src/common",
        "gui/src/model",
        "gui/src/screen1_screen",
        "generated/gui_generated/src/common",
        "generated/gui_generated/src/screen1_screen",
        "generated/images/src",
        "generated/images/src/__generated",
        "generated/fonts/src",
        "generated/texts/src",
    ] {
        touchgfx_rs::build::add_cpp_dir(&mut build, tg.join(relative));
    }

    build.compile("touchgfx_app");
    touchgfx_rs::build::link_core_library(&paths.core_library);
    touchgfx_rs::build::link_arm_runtime(&build);

    // App memory policy: newlib heap starts in initialized PSRAM.
    println!("cargo:rustc-link-arg-bins=--defsym=end=0x90400000");
    println!("cargo:rustc-link-arg-bins=--defsym=__exidx_start=0");
    println!("cargo:rustc-link-arg-bins=--defsym=__exidx_end=0");
    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
