cmake_minimum_required(VERSION 3.20)
project(TrajecSimuGUI CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Emit compile_commands.json so CLion / clangd can index all C++ sources.
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

set(RUST_CRATE_ROOT ${CMAKE_CURRENT_SOURCE_DIR}/crates/)
set(SIMULATOR_CORE ${CMAKE_CURRENT_SOURCE_DIR}/crates/simulator_core)
set(JSBSIM_ROOT    ${CMAKE_CURRENT_SOURCE_DIR}/jsbsim)

# ---------------------------------------------------------------------------
# JSBSim — same options as build.rs so CMake and Cargo stay in sync.
# ---------------------------------------------------------------------------
set(BUILD_SHARED_LIBS             OFF CACHE BOOL "" FORCE)
set(BUILD_PYTHON_MODULE           OFF CACHE BOOL "" FORCE)
set(INSTALL_JSBSIM_PYTHON_MODULE  OFF CACHE BOOL "" FORCE)
set(BUILD_DOCS                    OFF CACHE BOOL "" FORCE)
set(BUILD_JULIA_PACKAGE           OFF CACHE BOOL "" FORCE)
set(BUILD_MATLAB_SFUNCTION        OFF CACHE BOOL "" FORCE)
cmake_policy(SET CMP0077 NEW)   # honour normal-variable overrides of options()
add_subdirectory(${JSBSIM_ROOT} ${CMAKE_BINARY_DIR}/jsbsim)

# ---------------------------------------------------------------------------
# cxx-generated headers
#
# The cxx crate writes two headers into Cargo's OUT_DIR at build time:
#   rust/cxx.h                          — cxx runtime types
#   simulator_core/src/jsbsim/ffi.rs.h  — generated from ffi.rs
#
# Both land under:
#   target/{debug,release}/build/simulator_core-<hash>/out/cxxbridge/include/
#
# Run `cargo build` once before opening this CMake project so the headers
# exist.  You can override the path with -DCXX_BRIDGE_INCLUDE=<dir>.
# ---------------------------------------------------------------------------
if(NOT DEFINED CXX_BRIDGE_INCLUDE)
    file(GLOB _cxx_candidates
        "${CMAKE_CURRENT_SOURCE_DIR}/target/debug/build/simulator_core-*/out/cxxbridge/include"
        "${CMAKE_CURRENT_SOURCE_DIR}/target/release/build/simulator_core-*/out/cxxbridge/include"
    )
    if(_cxx_candidates)
        list(GET _cxx_candidates 0 CXX_BRIDGE_INCLUDE)
        message(STATUS "jsbsim_bridge: cxxbridge include → ${CXX_BRIDGE_INCLUDE}")
    else()
        message(WARNING
            "jsbsim_bridge: cxxbridge headers not found.\n"
            "Run 'cargo build' first, then re-run CMake.\n"
            "Or pass -DCXX_BRIDGE_INCLUDE=<path> explicitly."
        )
        set(CXX_BRIDGE_INCLUDE "")
    endif()
endif()

# ---------------------------------------------------------------------------
# jsbsim_bridge — the thin C++ wrapper around FGFDMExec
# ---------------------------------------------------------------------------
add_library(jsbsim_bridge STATIC
    crates/simulator_core/cpp/jsbsim_bridge.cpp
)

target_include_directories(jsbsim_bridge PUBLIC
    ${JSBSIM_ROOT}/src
    ${JSBSIM_ROOT}/src/simgear
    ${RUST_CRATE_ROOT}
    ${CMAKE_CURRENT_SOURCE_DIR}/target/cxxbridge/rust
)

if(CXX_BRIDGE_INCLUDE)
    target_include_directories(jsbsim_bridge PUBLIC ${CXX_BRIDGE_INCLUDE})
endif()

target_link_libraries(jsbsim_bridge PUBLIC libJSBSim)

target_compile_options(jsbsim_bridge PRIVATE
    $<$<CXX_COMPILER_ID:GNU,Clang>:-Wno-unused-parameter>
)