fn main() {
    // libgit2-sys on Windows needs advapi32 for security, registry, and crypto
    // APIs that libgit2 uses internally.
    if cfg!(windows) {
        println!("cargo:rustc-link-lib=advapi32");
    }
}
