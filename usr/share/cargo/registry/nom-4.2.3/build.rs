extern crate version_check;

fn main() {
  if version_check::is_min_version("1.26.0").unwrap_or(false) {
    println!("cargo:rustc-cfg=stable_i128");
  }
}
