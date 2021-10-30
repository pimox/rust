#![allow(dead_code)]

#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate regex_automata;
extern crate serde;
extern crate serde_bytes;
#[macro_use]
extern crate serde_derive;
extern crate toml;

#[cfg(feature = "std")]
mod collection;
#[cfg(feature = "std")]
mod regression;
#[cfg(feature = "std")]
mod suite;
mod unescape;
