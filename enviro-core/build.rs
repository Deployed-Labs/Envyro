use std::env;
use std::path::PathBuf;
use std::process::Command;

/// This build script orchestrates the compilation of:
/// 1. Zig components (C-ABI bridge for syscall wrapping and memory allocation)
/// 2. Go components (gRPC control plane and eBPF networking)
/// into a unified binary that Rust can link against.
fn main() {
    println!("cargo:rerun-if-changed=../enviro-zig/");
    println!("cargo:rerun-if-changed=../enviro-go/");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    
    // Build Zig components
    build_zig_components(&out_dir);
    
    // Build Go components
    build_go_components(&out_dir);
    
    // Link against the compiled libraries
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=enviro_zig");
    println!("cargo:rustc-link-lib=dylib=enviro_go");
}

/// Compiles Zig code into a static library with C-ABI compatibility.
/// Performance-First Pattern: Zig's manual memory management and zero-cost abstractions
/// provide optimal syscall wrapping without Rust's safety overhead.
fn build_zig_components(out_dir: &PathBuf) {
    let zig_dir = PathBuf::from("../enviro-zig");
    
    if !zig_dir.exists() {
        println!("cargo:warning=Zig directory not found, skipping Zig build");
        return;
    }

    let status = Command::new("zig")
        .args(&[
            "build-lib",
            "-static",
            "-target", "native-native",
            "-O", "ReleaseFast",  // Maximum performance
            "-femit-bin", &format!("{}/libenviro_zig.a", out_dir.display()),
            "../enviro-zig/src/oom_tuner.zig",
            "../enviro-zig/src/allocator.zig",
        ])
        .status();

    match status {
        Ok(status) if status.success() => {
            println!("cargo:info=Zig components built successfully");
        }
        Ok(status) => {
            println!("cargo:warning=Zig build failed with status: {}", status);
        }
        Err(e) => {
            println!("cargo:warning=Failed to execute Zig compiler: {}. Is Zig installed?", e);
        }
    }
}

/// Compiles Go code into a shared library using CGO.
/// Performance-First Pattern: Go's superior concurrency model and GC are ideal
/// for the control plane, while CGO allows us to expose Go functions to Rust.
fn build_go_components(out_dir: &PathBuf) {
    let go_dir = PathBuf::from("../enviro-go");
    
    if !go_dir.exists() {
        println!("cargo:warning=Go directory not found, skipping Go build");
        return;
    }

    // Set CGO flags for building shared library
    let status = Command::new("go")
        .args(&[
            "build",
            "-buildmode=c-shared",
            "-o", &format!("{}/libenviro_go.so", out_dir.display()),
            "./pkg/control",
        ])
        .current_dir(&go_dir)
        .env("CGO_ENABLED", "1")
        .status();

    match status {
        Ok(status) if status.success() => {
            println!("cargo:info=Go components built successfully");
        }
        Ok(status) => {
            println!("cargo:warning=Go build failed with status: {}", status);
        }
        Err(e) => {
            println!("cargo:warning=Failed to execute Go compiler: {}. Is Go installed?", e);
        }
    }
}
