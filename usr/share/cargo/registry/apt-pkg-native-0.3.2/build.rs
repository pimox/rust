extern crate cc;

use std::env;

const SRC: &str = "apt-pkg-c/lib.cpp";

fn main() {
    println!("cargo:rerun-if-changed={}", SRC);

    let mut build = cc::Build::new();
    build.file(SRC);
    build.cpp(true);
    build.flag("-std=gnu++11");

    #[cfg(feature = "ye-olde-apt")]
    {
        build.define("YE_OLDE_APT", "1");
    }

    build.compile("libapt-pkg-c.a");

    println!("dh-cargo:deb-built-using=apt-pkg-c=0={}", env::var("CARGO_MANIFEST_DIR").unwrap());
    println!("cargo:rustc-link-lib=apt-pkg");
}
