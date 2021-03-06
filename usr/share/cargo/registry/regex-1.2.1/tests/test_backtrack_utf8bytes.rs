#![cfg_attr(feature = "pattern", feature(pattern))]

extern crate rand;
extern crate regex;

macro_rules! regex_new {
    ($re:expr) => {{
        use regex::internal::ExecBuilder;
        ExecBuilder::new($re)
            .bounded_backtracking()
            .bytes(true)
            .build()
            .map(|e| e.into_regex())
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
            .bounded_backtracking()
            .bytes(true)
            .build()
            .map(|e| e.into_regex_set())
    }};
}

macro_rules! regex_set {
    ($res:expr) => {
        regex_set_new!($res).unwrap()
    };
}

// Must come before other module definitions.
include!("macros_str.rs");
include!("macros.rs");

mod api;
mod api_str;
mod crazy;
mod flags;
mod fowler;
mod multiline;
mod noparse;
mod regression;
mod replace;
mod searcher;
mod set;
mod suffix_reverse;
mod unicode;
mod word_boundary;
mod word_boundary_unicode;
