use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // JSBSim repo root (submodule at workspace root)
    let jsbsim = manifest.join("../../jsbsim");
    let jsbsim_src = jsbsim.join("src");

    // Pre-built static library (built separately with CMake)
    let jsbsim_lib = jsbsim.join("build/src");

    // --- cxx bridge ---
    cxx_build::bridge("src/jsbsim/ffi.rs")
        .file("cpp/jsbsim_bridge.cpp")
        // JSBSim public headers
        .include(&jsbsim_src)
        // simgear headers are under jsbsim/src/simgear
        .include(jsbsim_src.join("simgear"))
        // our own bridge header
        .include(manifest.join("cpp"))
        .flag_if_supported("-std=c++17")
        .flag_if_supported("-Wno-unused-parameter")
        .compile("jsbsim_bridge");

    // --- link JSBSim static lib ---
    println!(
        "cargo:rustc-link-search=native={}",
        jsbsim_lib.display()
    );
    println!("cargo:rustc-link-lib=static=JSBSim");

    // C++ stdlib (macOS uses libc++)
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=dylib=c++");
    #[cfg(not(target_os = "macos"))]
    println!("cargo:rustc-link-lib=dylib=stdc++");

    // Rebuild triggers
    println!("cargo:rerun-if-changed=cpp/jsbsim_bridge.h");
    println!("cargo:rerun-if-changed=cpp/jsbsim_bridge.cpp");
    println!("cargo:rerun-if-changed=src/jsbsim/ffi.rs");
}
