extern crate rand;
extern crate regex;

macro_rules! regex_new {
    ($re:expr) => {{
        use regex::internal::ExecBuilder;
        ExecBuilder::new($re)
            .nfa()
            .only_utf8(false)
            .build()
            .map(|e| e.into_byte_regex())
    }};
}

macro_rules! regex {
    ($re:expr) => {
        regex_new!($re).unwrap()
    };
}

macro_rules! regex_set_new {
    ($re:expr) => {{
        use regex::internal::ExecBuilder;
        ExecBuilder::new_many($re)
            .nfa()
            .only_utf8(false)
            .build()
            .map(|e| e.into_byte_regex_set())
    }};
}

macro_rules! regex_set {
    ($res:expr) => {
        regex_set_new!($res).unwrap()
    };
}

// Must come before other module definitions.
include!("macros_bytes.rs");
include!("macros.rs");

mod api;
mod bytes;
mod crazy;
mod flags;
mod fowler;
mod multiline;
mod noparse;
mod regression;
mod replace;
mod set;
mod suffix_reverse;
mod unicode;
mod word_boundary;
mod word_boundary_ascii;
