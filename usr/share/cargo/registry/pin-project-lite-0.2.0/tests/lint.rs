#![forbid(unsafe_code)]
#![warn(nonstandard_style, rust_2018_idioms, rustdoc, unused)]
// Note: This does not guarantee compatibility with forbidding these lints in the future.
// If rustc adds a new lint, we may not be able to keep this.
#![forbid(future_incompatible, rust_2018_compatibility)]
#![allow(unknown_lints)] // for old compilers
#![warn(
    box_pointers,
    deprecated_in_future,
    elided_lifetimes_in_paths,
    explicit_outlives_requirements,
    macro_use_extern_crate,
    meta_variable_misuse,
    missing_copy_implementations,
    missing_crate_level_docs,
    missing_debug_implementations,
    missing_docs,
    non_ascii_idents,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unaligned_references,
    unreachable_pub,
    unused_extern_crates,
    unused_import_braces,
    unused_lifetimes,
    unused_qualifications,
    unused_results,
    variant_size_differences
)]
// absolute_paths_not_starting_with_crate, anonymous_parameters, keyword_idents, pointer_structural_match: forbidden as a part of future_incompatible
// missing_doc_code_examples, private_doc_tests, invalid_html_tags: warned as a part of rustdoc
// unsafe_block_in_unsafe_fn: unstable
// unsafe_code: forbidden
// unstable_features: deprecated: https://doc.rust-lang.org/beta/rustc/lints/listing/allowed-by-default.html#unstable-features
// unused_crate_dependencies: unrelated
#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![warn(clippy::restriction)]
#![allow(clippy::blanket_clippy_restriction_lints)] // this is a test, so enable all restriction lints intentionally.

// Check interoperability with rustc and clippy lints.

pub mod basic {
    include!("include/basic.rs");
}

pub mod box_pointers {
    use pin_project_lite::pin_project;

    pin_project! {
        #[derive(Debug)]
        pub struct Struct {
            #[pin]
            pub p: Box<isize>,
            pub u: Box<isize>,
        }
    }

    pin_project! {
        #[project = EnumProj]
        #[project_ref = EnumProjRef]
        #[derive(Debug)]
        pub enum Enum {
            Struct {
                #[pin]
                p: Box<isize>,
                u: Box<isize>,
            },
            Unit,
        }
    }
}

pub mod explicit_outlives_requirements {
    use pin_project_lite::pin_project;

    pin_project! {
        #[derive(Debug)]
        pub struct Struct<'a, T, U>
        where
            T: ?Sized,
            U: ?Sized,
        {
            #[pin]
            pub pinned: &'a mut T,
            pub unpinned: &'a mut U,
        }
    }

    pin_project! {
        #[project = EnumProj]
        #[project_ref = EnumProjRef]
        #[derive(Debug)]
        pub enum Enum<'a, T, U>
        where
            T: ?Sized,
            U: ?Sized,
        {
            Struct {
                #[pin]
                pinned: &'a mut T,
                unpinned: &'a mut U,
            },
            Unit,
        }
    }
}

pub mod variant_size_differences {
    use pin_project_lite::pin_project;

    pin_project! {
        #[project = EnumProj]
        #[project_ref = EnumProjRef]
        #[allow(missing_debug_implementations, missing_copy_implementations)] // https://github.com/rust-lang/rust/pull/74060
        #[allow(variant_size_differences)] // for the type itself
        #[allow(clippy::large_enum_variant)] // for the type itself
        pub enum Enum {
            V1 { f: u8 },
            V2 { f: [u8; 1024] },
        }
    }
}

pub mod clippy_mut_mut {
    use pin_project_lite::pin_project;

    pin_project! {
        #[derive(Debug)]
        pub struct Struct<'a, T, U> {
            #[pin]
            pub pinned: &'a mut T,
            pub unpinned: &'a mut U,
        }
    }

    pin_project! {
        #[project = EnumProj]
        #[project_ref = EnumProjRef]
        #[derive(Debug)]
        pub enum Enum<'a, T, U> {
            Struct {
                #[pin]
                pinned: &'a mut T,
                unpinned: &'a mut U,
            },
            Unit,
        }
    }
}

#[allow(unreachable_pub)]
mod clippy_redundant_pub_crate {
    use pin_project_lite::pin_project;

    pin_project! {
        #[derive(Debug)]
        pub struct Struct<T, U> {
            #[pin]
            pub pinned: T,
            pub unpinned: U,
        }
    }

    #[allow(dead_code)]
    pin_project! {
        #[project = EnumProj]
        #[project_ref = EnumProjRef]
        #[derive(Debug)]
        pub enum Enum<T, U> {
            Struct {
                #[pin]
                pinned: T,
                unpinned: U,
            },
            Unit,
        }
    }
}

pub mod clippy_type_repetition_in_bounds {
    use pin_project_lite::pin_project;

    pin_project! {
        #[derive(Debug)]
        pub struct Struct<T, U>
        where
            Struct<T, U>: Sized,
        {
            #[pin]
            pub pinned: T,
            pub unpinned: U,
        }
    }

    pin_project! {
        #[project = EnumProj]
        #[project_ref = EnumProjRef]
        #[derive(Debug)]
        pub enum Enum<T, U>
        where
            Enum<T, U>: Sized,
        {
            Struct {
                #[pin]
                pinned: T,
                unpinned: U,
            },
            Unit,
        }
    }
}

pub mod clippy_used_underscore_binding {
    use pin_project_lite::pin_project;

    pin_project! {
        #[derive(Debug)]
        pub struct Struct<T, U> {
            #[pin]
            pub _pinned: T,
            pub _unpinned: U,
        }
    }

    pin_project! {
        #[project = EnumProj]
        #[project_ref = EnumProjRef]
        #[derive(Debug)]
        pub enum Enum<T, U> {
            Struct {
                #[pin]
                _pinned: T,
                _unpinned: U,
            },
        }
    }
}

#[allow(box_pointers)]
#[allow(clippy::restriction)]
#[rustversion::attr(before(2020-11-08), ignore)] // Note: This date is commit-date and the day before the toolchain date.
#[test]
fn check_lint_list() {
    use std::{env, fs, path::Path, process::Command, str};

    type Result<T, E = Box<dyn std::error::Error>> = std::result::Result<T, E>;

    fn assert_eq(expected_path: &str, actual: &str) -> Result<()> {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let expected_path = &manifest_dir.join(expected_path);
        let expected = fs::read_to_string(expected_path)?;
        if expected != actual {
            if env::var_os("CI").is_some() {
                let actual_path =
                    &manifest_dir.join("target").join(expected_path.file_name().unwrap());
                fs::write(actual_path, actual)?;
                let status = Command::new("git")
                    .args(&["--no-pager", "diff", "--no-index", "--"])
                    .args(&[expected_path, actual_path])
                    .status()?;
                assert!(!status.success());
                panic!("assertion failed");
            } else {
                fs::write(expected_path, actual)?;
            }
        }
        Ok(())
    }

    (|| -> Result<()> {
        let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let output = Command::new(rustc).args(&["-W", "help"]).output()?;
        let new = str::from_utf8(&output.stdout)?;
        assert_eq("tests/lint.txt", new)
    })()
    .unwrap_or_else(|e| panic!("{}", e));
}
