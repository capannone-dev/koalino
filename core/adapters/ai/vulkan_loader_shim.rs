//! Lets a Vulkan build of whisper.cpp start on a machine without Vulkan.
//!
//! ggml calls three Vulkan functions directly and reaches the rest through
//! `vkGetInstanceProcAddr`. Linking them from the Vulkan loader would make the operating system refuse
//! to load this library where the loader is missing, as on a machine with no GPU driver. So we define
//! those three here and open the loader only when called. Without a loader, every Vulkan function reports an error,
//! ggml registers no GPU, and transcription uses the CPU.

use std::{
    ffi::{CStr, c_char, c_void},
    sync::OnceLock,
};

/// Vulkan's type-erased function pointer.
type VulkanFunction = unsafe extern "system" fn();
/// The signature of the loader's own `vkGetInstanceProcAddr`.
type GetInstanceProcAddr =
    unsafe extern "system" fn(*mut c_void, *const c_char) -> Option<VulkanFunction>;

/// The signature of `vkCmdCopyBuffer`: command buffer, source, destination, count and regions.
type CmdCopyBuffer = unsafe extern "system" fn(*mut c_void, u64, u64, u32, *const c_void);
/// The signature of `vkGetPhysicalDeviceFeatures2`: physical device and output structure.
type GetPhysicalDeviceFeatures2 = unsafe extern "system" fn(*mut c_void, *mut c_void);

/// Vulkan's `VK_ERROR_INITIALIZATION_FAILED`.
const VULKAN_INITIALIZATION_FAILED: i32 = -3;

/// Stands in for every Vulkan function when there is no loader.
///
/// Outputs: `VK_ERROR_INITIALIZATION_FAILED`, which ggml treats as having no Vulkan. Callers pass
/// arguments that this ignores; both supported x86-64 calling conventions have the caller clean up.
/// Side effects: None.
unsafe extern "system" fn vulkan_unavailable() -> i32 {
    VULKAN_INITIALIZATION_FAILED
}

/// The loader's exports that ggml calls directly.
#[derive(Clone, Copy)]
struct LoaderEntries {
    get_instance_proc_addr: GetInstanceProcAddr,
    cmd_copy_buffer: CmdCopyBuffer,
    get_physical_device_features2: GetPhysicalDeviceFeatures2,
}

/// Returns the loader's exports, opening the loader on first use.
///
/// Outputs: The functions, or `None` when this machine has no complete Vulkan loader.
/// Side effects: Loads the system's Vulkan library once, and it stays loaded.
fn loader_entries() -> Option<LoaderEntries> {
    static LOADER_ENTRIES: OnceLock<Option<LoaderEntries>> = OnceLock::new();
    *LOADER_ENTRIES.get_or_init(|| {
        let library = open_vulkan_loader()?;
        let get_instance_proc_addr = loader_symbol(library, c"vkGetInstanceProcAddr")?;
        let cmd_copy_buffer = loader_symbol(library, c"vkCmdCopyBuffer")?;
        let get_physical_device_features2 =
            loader_symbol(library, c"vkGetPhysicalDeviceFeatures2")?;
        // SAFETY: Each symbol has the signature that the Vulkan specification gives it.
        unsafe {
            Some(LoaderEntries {
                get_instance_proc_addr: std::mem::transmute::<*mut c_void, GetInstanceProcAddr>(
                    get_instance_proc_addr,
                ),
                cmd_copy_buffer: std::mem::transmute::<*mut c_void, CmdCopyBuffer>(cmd_copy_buffer),
                get_physical_device_features2: std::mem::transmute::<
                    *mut c_void,
                    GetPhysicalDeviceFeatures2,
                >(get_physical_device_features2),
            })
        }
    })
}

/// Opens the Vulkan loader on Linux.
///
/// Outputs: Its handle, or `None` when absent. Side effects: Loads the library.
#[cfg(unix)]
fn open_vulkan_loader() -> Option<*mut c_void> {
    // SAFETY: The name is NUL-terminated.
    let library = unsafe {
        libc::dlopen(
            c"libvulkan.so.1".as_ptr(),
            libc::RTLD_NOW | libc::RTLD_LOCAL,
        )
    };
    (!library.is_null()).then_some(library)
}

/// Finds an export of the Vulkan loader on Linux.
///
/// Inputs: The loader's handle and the export's name. Outputs: Its address, or `None`.
/// Side effects: None.
#[cfg(unix)]
fn loader_symbol(library: *mut c_void, name: &CStr) -> Option<*mut c_void> {
    // SAFETY: The handle came from `dlopen`, and the name is NUL-terminated.
    let symbol = unsafe { libc::dlsym(library, name.as_ptr()) };
    (!symbol.is_null()).then_some(symbol)
}

#[cfg(windows)]
unsafe extern "system" {
    fn LoadLibraryA(name: *const c_char) -> *mut c_void;
    fn GetProcAddress(library: *mut c_void, name: *const c_char) -> *mut c_void;
}

/// Opens the Vulkan loader on Windows.
///
/// Outputs: Its handle, or `None` when absent. Side effects: Loads the library.
#[cfg(windows)]
fn open_vulkan_loader() -> Option<*mut c_void> {
    // SAFETY: The name is NUL-terminated.
    let library = unsafe { LoadLibraryA(c"vulkan-1.dll".as_ptr()) };
    (!library.is_null()).then_some(library)
}

/// Finds an export of the Vulkan loader on Windows.
///
/// Inputs: The loader's handle and the export's name. Outputs: Its address, or `None`.
/// Side effects: None.
#[cfg(windows)]
fn loader_symbol(library: *mut c_void, name: &CStr) -> Option<*mut c_void> {
    // SAFETY: The handle came from `LoadLibraryA`, and the name is NUL-terminated.
    let symbol = unsafe { GetProcAddress(library, name.as_ptr()) };
    (!symbol.is_null()).then_some(symbol)
}

/// Replaces the Vulkan loader's export of the same name, which ggml links against.
///
/// Inputs: A Vulkan instance or null, and a NUL-terminated function name, as Vulkan defines them.
/// Outputs: The real function, or a stand-in that reports an error when there is no loader.
/// Side effects: May load the Vulkan library.
///
/// # Safety
/// Follows the Vulkan contract for `vkGetInstanceProcAddr`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkGetInstanceProcAddr(
    instance: *mut c_void,
    name: *const c_char,
) -> Option<VulkanFunction> {
    match loader_entries() {
        // SAFETY: The caller keeps the Vulkan contract, and this forwards its arguments unchanged.
        Some(entries) => unsafe { (entries.get_instance_proc_addr)(instance, name) },
        // SAFETY: Only the return type differs; see `vulkan_unavailable`.
        None => Some(unsafe {
            std::mem::transmute::<unsafe extern "system" fn() -> i32, VulkanFunction>(
                vulkan_unavailable,
            )
        }),
    }
}

/// Replaces the Vulkan loader's export of the same name, which ggml links against.
///
/// Inputs: As Vulkan defines them. Outputs: None. Side effects: Records a copy in the command buffer.
/// Without a loader there are no command buffers, so the call can't happen and does nothing.
///
/// # Safety
/// Follows the Vulkan contract for `vkCmdCopyBuffer`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkCmdCopyBuffer(
    command_buffer: *mut c_void,
    source_buffer: u64,
    destination_buffer: u64,
    region_count: u32,
    regions: *const c_void,
) {
    if let Some(entries) = loader_entries() {
        // SAFETY: The caller keeps the Vulkan contract, and this forwards its arguments unchanged.
        unsafe {
            (entries.cmd_copy_buffer)(
                command_buffer,
                source_buffer,
                destination_buffer,
                region_count,
                regions,
            );
        }
    }
}

/// Replaces the Vulkan loader's export of the same name, which ggml links against.
///
/// Inputs: As Vulkan defines them. Outputs: Fills `features`. Side effects: None. Without a loader
/// there are no physical devices, so the call can't happen and does nothing.
///
/// # Safety
/// Follows the Vulkan contract for `vkGetPhysicalDeviceFeatures2`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkGetPhysicalDeviceFeatures2(
    physical_device: *mut c_void,
    features: *mut c_void,
) {
    if let Some(entries) = loader_entries() {
        // SAFETY: The caller keeps the Vulkan contract, and this forwards its arguments unchanged.
        unsafe { (entries.get_physical_device_features2)(physical_device, features) };
    }
}
