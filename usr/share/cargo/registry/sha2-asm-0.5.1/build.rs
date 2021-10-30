extern crate cc;

use std::env;

fn main() {
    let (sha256_path, sha512_path) = if cfg!(target_arch = "x86") {
        ("src/sha256_x86.S", "src/sha512_x86.S")
    } else if cfg!(target_arch = "x86_64") {
        ("src/sha256_x64.S", "src/sha512_x64.S")
    } else {
        panic!("Unsupported target architecture");
    };
    cc::Build::new()
              .flag("-c")
              .file(sha256_path)
              .compile("libsha256.a");
    cc::Build::new()
              .flag("-c")
              .file(sha512_path)
              .compile("libsha512.a");

    println!("dh-cargo:deb-built-using=sha256=0={}", env::var("CARGO_MANIFEST_DIR").unwrap());
    println!("dh-cargo:deb-built-using=sha512=0={}", env::var("CARGO_MANIFEST_DIR").unwrap());
}
