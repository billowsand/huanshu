//! Windows gives the main thread a 1 MiB stack. Unoptimised builds keep large stack frames
//! (GPUI element trees are built in deeply nested calls), and the third step's editor form
//! overflowed it in debug builds. Raise it to 8 MiB for every Windows MSVC build.

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("windows") && target.contains("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
    }
}
