use std::ffi::{CString, OsStr};
use std::slice;

use libc::{c_char, c_int};

use std::os::unix::prelude::*;

use error;

use Result;

pub fn ptr_to_os_str<'a>(ptr: *const c_char) -> Option<&'a OsStr> {
    if ptr.is_null() {
        return None;
    }

    Some(unsafe { ptr_to_os_str_unchecked(ptr) })
}

pub unsafe fn ptr_to_os_str_unchecked<'a>(ptr: *const c_char) -> &'a OsStr {
    OsStr::from_bytes(slice::from_raw_parts(
        ptr as *const u8,
        libc::strlen(ptr) as usize,
    ))
}

pub fn os_str_to_cstring<T: AsRef<OsStr>>(s: T) -> Result<CString> {
    match CString::new(s.as_ref().as_bytes()) {
        Ok(s) => Ok(s),
        Err(_) => Err(error::from_errno(libc::EINVAL)),
    }
}

pub fn errno_to_result(errno: c_int) -> Result<()> {
    match errno {
        0 => Ok(()),
        e => Err(error::from_errno(e)),
    }
}
