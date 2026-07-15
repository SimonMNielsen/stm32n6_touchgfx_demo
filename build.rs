//! Build script for stm32n6_touchgfx_demo.
//!
//! 1. Emits the RAM-app memory map (memory_ram.x) + linker args.
//! 2. Compiles the TouchGFX application C++ (GUI screens, generated assets)
//!    from the vendored project in `touchgfx_project/`, plus this project's
//!    C++ glue, together with the cxx bridge.
//! 3. Links the prebuilt TouchGFX core library (native Cortex-M55 build).

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

/// The TouchGFX STM32CubeIDE project (GUI + assets + framework), vendored into
/// this repo at `touchgfx_project/`. The application lives in the Appli half of
/// the FSBL/Appli split (800×480 RGB565, Cortex-M55 core lib, ferris/LED demo).
///
/// Relative to CARGO_MANIFEST_DIR so the build is location-independent — see
/// `tgfx_app()`.
const TGFX_APP_REL: &str = "touchgfx_project/Appli";

/// Flags that select the multilib variant. These must match what the C++ is
/// actually compiled with, or the compiler would report libraries for a
/// different float ABI / architecture than the objects we link against.
const MULTILIB_SELECT_FLAGS: &[&str] = &[
    "-march=armv8-m.main+fp.dp",
    "-mthumb",
    "-mfloat-abi=hard",
    "-mfpu=fpv5-d16",
];

/// Add link-search paths for the Arm GNU Toolchain's newlib + libgcc, which
/// the compiled C++ pulls in (memcpy/memset, soft-float helpers, the libstdc++
/// bits cxx's runtime drags in).
///
/// Rather than hardcoding an install path — which also means hardcoding the
/// gcc version and the `thumb/v8-m.main+dp/hard` multilib triple, all three of
/// which break on a toolchain update or a different machine — ask the compiler
/// itself with `-print-file-name`. It resolves the correct multilib for the
/// flags above, so this follows whatever `arm-none-eabi-g++` cc has picked
/// (PATH, or a CC/CXX override).
fn add_toolchain_lib_search_paths(b: &cc::Build) {
    let compiler = b.get_compiler();
    let cc_path = compiler.path();

    let mut dirs: Vec<PathBuf> = Vec::new();
    // libc/libm/libnosys come from newlib, libgcc from gcc — on some installs
    // these live in different directories, so resolve each and de-duplicate.
    for lib in ["libc.a", "libm.a", "libnosys.a", "libgcc.a"] {
        let out = std::process::Command::new(cc_path)
            .args(MULTILIB_SELECT_FLAGS)
            .arg(format!("-print-file-name={lib}"))
            .output()
            .unwrap_or_else(|e| panic!("running {cc_path:?} -print-file-name={lib}: {e}"));

        let printed = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let path = PathBuf::from(&printed);
        // gcc echoes the bare filename back when it can't find the library.
        assert!(
            path.is_absolute() && path.is_file(),
            "{cc_path:?} could not locate {lib} for {MULTILIB_SELECT_FLAGS:?} (got {printed:?}).\n\
             Is the Arm GNU Toolchain (arm-none-eabi-g++) on PATH?"
        );
        let dir = path.parent().expect("library has a parent dir").to_path_buf();
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }

    for dir in dirs {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }
}

/// Absolute path to the vendored TouchGFX application, derived from the crate
/// root. Uses forward slashes so it can be pasted into the `-I`/link flags
/// unchanged on Windows.
fn tgfx_app() -> String {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    format!("{}/{}", manifest.replace('\\', "/"), TGFX_APP_REL)
}

fn add_cpp_dir(build: &mut cc::Build, dir: &str) {
    let mut found = false;
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {dir}: {e}")) {
        let path = entry.unwrap().path();
        if path.extension().map_or(false, |e| e == "cpp") {
            build.file(&path);
            found = true;
        }
    }
    assert!(found, "no .cpp files in {dir}");
}

fn main() {
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());

    // ── Memory map (RAM app only) ─────────────────────────────────────────
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory_ram.x"))
        .unwrap();
    File::create(out.join("touchgfx_ctors.x"))
        .unwrap()
        .write_all(include_bytes!("touchgfx_ctors.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory_ram.x");
    println!("cargo:rerun-if-changed=touchgfx_ctors.x");
    println!("cargo:rerun-if-changed=cpp");
    println!("cargo:rerun-if-changed=src/bridge.rs");
    let tgfx_app = tgfx_app();
    let tg = format!("{tgfx_app}/TouchGFX");

    // Recompile when the TouchGFX GUI user code changes — the Screen1View
    // bounce/LED logic lives there, and the Designer rewrites these files.
    println!("cargo:rerun-if-changed={tg}/gui/src");
    println!("cargo:rerun-if-changed={tg}/gui/include");
    // ...and when the Designer regenerates widgets/assets/texts.
    println!("cargo:rerun-if-changed={tg}/generated");

    // ── TouchGFX C++ (bridge + glue + application) ────────────────────────
    let mut b = cxx_build::bridge("src/bridge.rs");
    b.cpp(true)
        .std("c++14")
        .include("cpp")
        .include(format!("{tgfx_app}/Middlewares/ST/touchgfx/framework/include"))
        // For CortexMMCUInstrumentation.hpp (MCU-load measurement). Only that
        // one file is taken from the project's target/ dir — the rest of it is
        // ST-HAL/NemaGFX bound and replaced by cpp/.
        .include(format!("{tg}/target"))
        .include(format!("{tg}/gui/include"))
        .include(format!("{tg}/generated/gui_generated/include"))
        .include(format!("{tg}/generated/fonts/include"))
        .include(format!("{tg}/generated/images/include"))
        .include(format!("{tg}/generated/texts/include"))
        // cc supplies -march=armv8-m.main+fp.dp -mthumb -mfloat-abi=hard for
        // this target; -mtune only tunes M55 scheduling (see demos build.rs).
        .flag("-mtune=cortex-m55")
        .flag("-mfpu=fpv5-d16")
        .flag("-Os")
        .flag("-fno-exceptions")
        .flag("-fno-rtti")
        .flag("-fno-threadsafe-statics")
        .flag("-fdata-sections")
        .flag("-ffunction-sections")
        .flag("-Wno-psabi")
        .flag("-Wno-unused-parameter")
        .define("USE_BPP", "16")
        .cpp_link_stdlib(None);

    // This project's glue (HAL, OSWrappers, touch, config, C++ runtime stubs).
    add_cpp_dir(&mut b, "cpp");

    // TouchGFX's own DWT-based MCU-load instrumentation (no ST HAL: it reads
    // the cycle counter at 0xE0001004 directly; main() enables the counter).
    b.file(format!("{tg}/target/CortexMMCUInstrumentation.cpp"));

    // TouchGFX application: user GUI + generated screens/assets.
    add_cpp_dir(&mut b, &format!("{tg}/gui/src/common"));
    add_cpp_dir(&mut b, &format!("{tg}/gui/src/model"));
    add_cpp_dir(&mut b, &format!("{tg}/gui/src/screen1_screen"));
    add_cpp_dir(&mut b, &format!("{tg}/generated/gui_generated/src/common"));
    add_cpp_dir(&mut b, &format!("{tg}/generated/gui_generated/src/screen1_screen"));
    add_cpp_dir(&mut b, &format!("{tg}/generated/images/src"));
    add_cpp_dir(&mut b, &format!("{tg}/generated/images/src/__generated"));
    add_cpp_dir(&mut b, &format!("{tg}/generated/fonts/src"));
    add_cpp_dir(&mut b, &format!("{tg}/generated/texts/src"));

    b.compile("touchgfx_app");

    // ── TouchGFX core library (native Cortex-M55 build) ──────────────────
    println!(
        "cargo:rustc-link-search=native={tgfx_app}/Middlewares/ST/touchgfx/lib/core/cortex_m55/gcc"
    );
    println!("cargo:rustc-link-lib=static=touchgfx-float-abi-hard");

    // ── C runtime (newlib) for the C++ objects ────────────────────────────
    // Ask the cross-compiler where its libraries actually are, rather than
    // hardcoding an install path + gcc version + multilib triple.
    add_toolchain_lib_search_paths(&b);
    println!("cargo:rustc-link-lib=c");
    println!("cargo:rustc-link-lib=m");
    println!("cargo:rustc-link-lib=nosys");
    println!("cargo:rustc-link-lib=gcc");

    // newlib link-script symbols: `end` = _sbrk heap base. The libstdc++
    // locale ctors (dragged in by cxx's runtime) malloc a little at startup;
    // heap lives in PSRAM, which main() initialises before running the C++
    // constructors. Framebuffers end at 0x9018_0000+750K < 0x9040_0000.
    println!("cargo:rustc-link-arg-bins=--defsym=end=0x90400000");
    println!("cargo:rustc-link-arg-bins=--defsym=__exidx_start=0");
    println!("cargo:rustc-link-arg-bins=--defsym=__exidx_end=0");

    // ── Standard cortex-m-rt link arguments + C++ ctor table ──────────────
    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
    println!("cargo:rustc-link-arg-bins=-Ttouchgfx_ctors.x");
}
