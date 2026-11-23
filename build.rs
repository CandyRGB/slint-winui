fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");
}
