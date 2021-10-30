#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![no_std]

extern crate libc;

#[cfg(feature = "std")]
extern crate std;

// If running bindgen, we'll end up with the correct bindings anyway.
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
