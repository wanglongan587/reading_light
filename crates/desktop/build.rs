fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // GPUI lays out nested content on the UI thread. A larger reserve keeps
        // long, nested reader and echo views stable without changing behavior.
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
    }
}
