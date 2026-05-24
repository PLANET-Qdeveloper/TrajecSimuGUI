use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let jsbsim = manifest.join("../../jsbsim");

    if !jsbsim.join("CMakeLists.txt").exists() {
        panic!(
            "JSBSim submodule not found at {}. Run: git submodule update --init --recursive",
            jsbsim.display()
        );
    }

    // Build JSBSim via its own CMake. The `cmake` crate maps Cargo's
    // OPT_LEVEL / DEBUG onto CMAKE_BUILD_TYPE:
    //   OPT_LEVEL=0           -> Debug                (matches `dev`)
    //   OPT_LEVEL>=1, !DEBUG  -> Release              (matches `release`)
    //   OPT_LEVEL>=1,  DEBUG  -> RelWithDebInfo       (matches `profiling`)
    //   OPT_LEVEL=s|z         -> MinSizeRel
    let dst = cmake::Config::new(&jsbsim)
        .define("BUILD_SHARED_LIBS", "OFF")
        // Disable everything we don't consume from Rust.
        .define("BUILD_PYTHON_MODULE", "OFF")
        .define("INSTALL_JSBSIM_PYTHON_MODULE", "OFF")
        .define("BUILD_DOCS", "OFF")
        .define("BUILD_JULIA_PACKAGE", "OFF")
        .define("BUILD_MATLAB_SFUNCTION", "OFF")
        // Skip optional tool discovery so a stock dev box (no Cython /
        // Doxygen / CxxTest) doesn't trigger configure-time work.
        .define("CMAKE_DISABLE_FIND_PACKAGE_Doxygen", "ON")
        .define("CMAKE_DISABLE_FIND_PACKAGE_CxxTest", "ON")
        .define("CMAKE_DISABLE_FIND_PACKAGE_Cython", "ON")
        .define("CMAKE_DISABLE_FIND_PACKAGE_Python3", "ON")
        .build();

    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());
    let lib_dir = {
        let base = dst.join("lib");
        let sub = base.join(&profile); // lib/debug, lib/release (Unix cmake)
        // MSVC cmake outputs to lib/Release or lib/Debug (title-case)
        let mut title = profile.clone();
        if let Some(first) = title.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        let sub_title = base.join(&title);
        if sub.exists() {
            sub
        } else if sub_title.exists() {
            sub_title
        } else {
            base
        }
    };
    let header_dir = jsbsim.join("src");

    cxx_build::bridge("src/jsbsim/ffi.rs")
        .file("cpp/jsbsim_bridge.cpp")
        .include(&header_dir)
        .include(header_dir.join("simgear"))
        .include(manifest.join("cpp"))
        .define("JSBSIM_STATIC_LINK", None) // suppress __declspec(dllimport) on Windows static link
        .flag_if_supported("-std=c++17")
        .flag_if_supported("-Wno-unused-parameter")
        .compile("jsbsim_bridge");

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=JSBSim");

    // Embed an rpath so the consuming binary (simulator_cli, src-tauri, …)
    // can find libJSBSim at runtime without DYLD_/LD_LIBRARY_PATH gymnastics.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match target_os.as_str() {
        "macos" => {
            println!("cargo:rustc-link-lib=dylib=c++");
        }
        "linux" => {
            println!("cargo:rustc-link-lib=dylib=stdc++");
        }
        // Windows: no rpath; ship JSBSim.dll next to the .exe at packaging time.
        _ => {
            if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
                println!("cargo:rustc-link-lib=dylib=stdc++");
            }
        }
    }

    println!("cargo:rerun-if-changed=cpp/jsbsim_bridge.h");
    println!("cargo:rerun-if-changed=cpp/jsbsim_bridge.cpp");
    println!("cargo:rerun-if-changed=src/jsbsim/ffi.rs");
    println!(
        "cargo:rerun-if-changed={}",
        manifest.join("../../.git/modules/jsbsim/HEAD").display()
    );
}
