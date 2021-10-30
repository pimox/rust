extern crate bindgen;

extern crate pkg_config;

use std::path::PathBuf;
use std::{env, fs};

fn generate_bindings(defs: Vec<&str>, headerpaths: Vec<PathBuf>) {
    use std::env;
    use std::path::PathBuf;

    let bindings = bindgen::Builder::default()
        .header("zstd.h")
        .blacklist_type("max_align_t")
        .size_t_is_usize(true)
        .use_core()
        .rustified_enum(".*")
        .clang_args(
            headerpaths
                .into_iter()
                .map(|path| format!("-I{}", path.display())),
        )
        .clang_args(defs.into_iter().map(|def| format!("-D{}", def)));

    #[cfg(feature = "experimental")]
    let bindings = bindings.clang_arg("-DZSTD_STATIC_LINKING_ONLY");

    #[cfg(not(feature = "std"))]
    let bindings = bindings.ctypes_prefix("libc");

    let bindings = bindings.generate().expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Could not write bindings");
}

fn pkg_config() -> (Vec<&'static str>, Vec<PathBuf>) {
    let library = pkg_config::Config::new()
        .statik(true)
        .cargo_metadata(!cfg!(feature = "non-cargo"))
        .probe("libzstd")
        .expect("Can't probe for zstd in pkg-config");
    (vec!["PKG_CONFIG"], library.include_paths)
}

fn main() {
    // println!("cargo:rustc-link-lib=zstd");

    let (defs, headerpaths) = pkg_config();

    generate_bindings(defs, headerpaths);
}
