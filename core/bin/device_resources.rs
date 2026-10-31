//! Reads free disk space and total memory, so Settings can warn before a large model download.

use std::path::Path;

/// Returns the bytes available to this app on the disk holding `path`.
///
/// Inputs: An existing folder. Outputs: The bytes, or `None` when the system cannot say.
/// Side effects: None.
#[cfg(unix)]
pub fn available_disk_bytes(path: &Path) -> Option<u64> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let native_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: Both pointers are valid for this call, and `stats` is read only on success.
    let status = unsafe { libc::statvfs(native_path.as_ptr(), stats.as_mut_ptr()) };
    if status != 0 {
        return None;
    }
    // SAFETY: `statvfs` returned success, so it filled the struct.
    let stats = unsafe { stats.assume_init() };
    #[allow(clippy::useless_conversion, clippy::unnecessary_cast)]
    Some(u64::from(stats.f_bavail) * stats.f_frsize as u64)
}

/// Returns the bytes available to this app on the disk holding `path`.
///
/// Inputs: An existing folder. Outputs: The bytes, or `None` when the system cannot say.
/// Side effects: None.
#[cfg(windows)]
pub fn available_disk_bytes(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    unsafe extern "system" {
        fn GetDiskFreeSpaceExW(
            directory: *const u16,
            available: *mut u64,
            total: *mut u64,
            free: *mut u64,
        ) -> i32;
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available = 0_u64;
    // SAFETY: The string is NUL-terminated and the output pointer is valid; the others may be null.
    let status = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (status != 0).then_some(available)
}

/// Returns the machine's total memory.
///
/// Outputs: The bytes, or `None` when the system cannot say. Side effects: None.
#[cfg(unix)]
pub fn total_memory_bytes() -> Option<u64> {
    // SAFETY: `sysconf` takes no pointers and only reads system values.
    let (pages, page_size) = unsafe {
        (
            libc::sysconf(libc::_SC_PHYS_PAGES),
            libc::sysconf(libc::_SC_PAGESIZE),
        )
    };
    if pages <= 0 || page_size <= 0 {
        return None;
    }
    Some(u64::try_from(pages).ok()? * u64::try_from(page_size).ok()?)
}

/// Returns the machine's total memory.
///
/// Outputs: The bytes, or `None` when the system cannot say. Side effects: None.
#[cfg(windows)]
pub fn total_memory_bytes() -> Option<u64> {
    #[repr(C)]
    struct MemoryStatus {
        length: u32,
        memory_load: u32,
        total_physical: u64,
        available_physical: u64,
        total_page_file: u64,
        available_page_file: u64,
        total_virtual: u64,
        available_virtual: u64,
        available_extended_virtual: u64,
    }
    unsafe extern "system" {
        fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
    }
    let mut status = MemoryStatus {
        length: u32::try_from(std::mem::size_of::<MemoryStatus>()).ok()?,
        memory_load: 0,
        total_physical: 0,
        available_physical: 0,
        total_page_file: 0,
        available_page_file: 0,
        total_virtual: 0,
        available_virtual: 0,
        available_extended_virtual: 0,
    };
    // SAFETY: The pointer is valid and `length` is set as the API requires.
    (unsafe { GlobalMemoryStatusEx(&mut status) } != 0).then_some(status.total_physical)
}
