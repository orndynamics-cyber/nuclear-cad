//! The user's Documents folder as Windows reports it (`SHGetKnownFolderPath`), so the project
//! library follows a Documents folder that was moved or redirected to OneDrive instead of a path
//! built from the user's name. There is no safe API for this; the one call and the buffer it
//! returns are handled here.
#![allow(unsafe_code)]

use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::{FOLDERID_Documents, KF_FLAG_DEFAULT, SHGetKnownFolderPath};

/// The Documents folder, or `None` when Windows cannot say (the caller then asks the user).
pub fn documents() -> Option<PathBuf> {
    let mut path: windows_sys::core::PWSTR = std::ptr::null_mut();
    // SAFETY: the GUID is a static; a null token asks for the current user; on return `path`
    // is null or a NUL-terminated buffer the shell allocated, which this function frees with
    // CoTaskMemFree whatever the result (the documented contract, even on failure).
    let hr = unsafe { SHGetKnownFolderPath(&FOLDERID_Documents, KF_FLAG_DEFAULT as u32, std::ptr::null_mut(), &mut path) };
    if path.is_null() {
        return None;
    }
    let mut len = 0usize;
    // SAFETY: `path` is NUL-terminated (see above); the walk stops at the NUL, and at a bound far
    // beyond any Windows path so a damaged buffer cannot run away.
    while len < 32_768 && unsafe { *path.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: the first `len` units were just read one by one.
    let wide = unsafe { std::slice::from_raw_parts(path, len) }.to_vec();
    // SAFETY: allocated by the shell for this call and not used after this.
    unsafe { CoTaskMemFree(path as *const core::ffi::c_void) };
    if hr < 0 || wide.is_empty() {
        return None;
    }
    Some(PathBuf::from(std::ffi::OsString::from_wide(&wide)))
}
