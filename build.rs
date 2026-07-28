use std::env;

const WINDOWS_STACK_RESERVE_BYTES: usize = 8 * 1024 * 1024;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let argument = match env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("msvc") => format!("/STACK:{WINDOWS_STACK_RESERVE_BYTES}"),
        Ok("gnu") => format!("-Wl,--stack,{WINDOWS_STACK_RESERVE_BYTES}"),
        _ => return,
    };

    // Windows executables reserve only 1 MiB for the main thread by default.
    // Code startup has a deep, finite async initialization path which exceeds
    // that limit. Reserving address space here matches the common Unix 8 MiB
    // main-thread stack without committing all of it up front.
    println!("cargo:rustc-link-arg-bin=a3s={argument}");
}
