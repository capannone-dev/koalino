//! Lets an OpenCL build of whisper.cpp start on a phone without OpenCL.
//!
//! Not every phone has `libOpenCL.so`, and linking it would stop the whole app loading on those
//! that don't. So we define the functions ggml calls and open the library on first use. Without it,
//! or with a GPU that efficient mode doesn't support, `clGetPlatformIDs` reports an error, ggml
//! registers no GPU, and transcription uses the CPU. The other functions can only be reached after
//! ggml has found a device. The Android manifest declares the library, which is what lets an app see
//! the vendor's copy at all.

use std::{
    ffi::{CStr, c_void},
    sync::OnceLock,
};

use super::compute_resources::is_efficient_gpu;

/// An OpenCL handle or a pointer argument, which is only passed through.
type Pointer = *mut c_void;

/// OpenCL's `CL_PLATFORM_NOT_FOUND_KHR`, returned when the library or function is missing.
const OPENCL_UNAVAILABLE: i32 = -1001;

/// Opens the OpenCL library once.
///
/// Outputs: Its handle as an address, or `None` when this phone has none.
/// Side effects: Loads the library, which stays loaded.
fn opencl_library() -> Option<usize> {
    static LIBRARY: OnceLock<Option<usize>> = OnceLock::new();
    *LIBRARY.get_or_init(|| {
        // SAFETY: The name is NUL-terminated.
        let library =
            unsafe { libc::dlopen(c"libOpenCL.so".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
        (!library.is_null()).then_some(library as usize)
    })
}

/// Finds a function in the OpenCL library.
///
/// Inputs: The function's name. Outputs: Its address, or `None` when missing.
/// Side effects: See `opencl_library`.
fn opencl_symbol(name: &CStr) -> Option<usize> {
    // SAFETY: The handle came from `dlopen`, and the name is NUL-terminated.
    let symbol = unsafe { libc::dlsym(opencl_library()? as Pointer, name.as_ptr()) };
    (!symbol.is_null()).then_some(symbol as usize)
}

/// The signature of `clGetPlatformIDs`.
type GetPlatformIds = unsafe extern "C" fn(u32, Pointer, *mut u32) -> i32;
/// The signature of `clGetDeviceIDs`.
type GetDeviceIds = unsafe extern "C" fn(Pointer, u64, u32, *mut Pointer, *mut u32) -> i32;
/// The signature of `clGetDeviceInfo`.
type GetDeviceInfo = unsafe extern "C" fn(Pointer, u32, usize, *mut c_void, *mut usize) -> i32;

/// OpenCL's `CL_DEVICE_TYPE_GPU`.
const OPENCL_DEVICE_TYPE_GPU: u64 = 4;
/// OpenCL's `CL_DEVICE_VERSION`.
const OPENCL_DEVICE_VERSION: u32 = 0x102F;
/// Most platforms or devices to look through.
const MOST_HANDLES: usize = 16;

/// Finds a GPU that efficient mode supports, once.
///
/// Outputs: Its version text, or `None`. This only reads what the driver reports: it compiles nothing.
/// Side effects: Loads the OpenCL library if this phone has one.
pub fn efficient_gpu_version() -> Option<&'static str> {
    static VERSION: OnceLock<Option<String>> = OnceLock::new();
    VERSION.get_or_init(find_efficient_gpu).as_deref()
}

/// Looks through the driver's GPUs. See `efficient_gpu_version`.
///
/// Outputs: The version text of the first supported GPU, or `None`.
/// Side effects: Calls the driver's query functions.
fn find_efficient_gpu() -> Option<String> {
    // SAFETY: Each address is that function's, whose signature OpenCL defines as here. Each array has
    // the size passed, and each out-pointer is valid for the call.
    unsafe {
        let get_platforms =
            std::mem::transmute::<usize, GetPlatformIds>(opencl_symbol(c"clGetPlatformIDs")?);
        let get_devices =
            std::mem::transmute::<usize, GetDeviceIds>(opencl_symbol(c"clGetDeviceIDs")?);
        let get_info =
            std::mem::transmute::<usize, GetDeviceInfo>(opencl_symbol(c"clGetDeviceInfo")?);
        let mut platforms = [std::ptr::null_mut::<c_void>(); MOST_HANDLES];
        let mut platform_count = 0;
        if get_platforms(
            MOST_HANDLES as u32,
            platforms.as_mut_ptr().cast(),
            &mut platform_count,
        ) != 0
        {
            return None;
        }
        for &platform in platforms.iter().take(platform_count as usize) {
            let mut devices = [std::ptr::null_mut::<c_void>(); MOST_HANDLES];
            let mut device_count = 0;
            if get_devices(
                platform,
                OPENCL_DEVICE_TYPE_GPU,
                MOST_HANDLES as u32,
                devices.as_mut_ptr(),
                &mut device_count,
            ) != 0
            {
                continue;
            }
            for &device in devices.iter().take(device_count as usize) {
                let mut text = [0_u8; 128];
                if get_info(
                    device,
                    OPENCL_DEVICE_VERSION,
                    text.len(),
                    text.as_mut_ptr().cast(),
                    std::ptr::null_mut(),
                ) != 0
                {
                    continue;
                }
                let version = CStr::from_bytes_until_nul(&text).ok()?.to_string_lossy();
                if is_efficient_gpu(&version) {
                    return Some(version.into_owned());
                }
            }
        }
    }
    None
}

/// Defines a function that forwards to the library's, looked up once, or reports it unavailable.
macro_rules! forward_status {
    ($name:ident($($argument:ident: $kind:ty),* $(,)?)) => {
        /// Replaces the OpenCL library's export of the same name, which ggml links against.
        ///
        /// Inputs: As OpenCL defines them. Outputs: OpenCL's status, or `OPENCL_UNAVAILABLE`.
        /// Side effects: Those of the OpenCL function; may load the library.
        ///
        /// # Safety
        /// Follows the OpenCL contract for this function.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name($($argument: $kind),*) -> i32 {
            static SYMBOL: OnceLock<Option<usize>> = OnceLock::new();
            let symbol = *SYMBOL.get_or_init(|| opencl_symbol(
                // SAFETY: The literal ends in a NUL byte.
                unsafe { CStr::from_bytes_with_nul_unchecked(concat!(stringify!($name), "\0").as_bytes()) }
            ));
            match symbol {
                // SAFETY: The address is this function's, whose signature OpenCL defines as here.
                Some(address) => unsafe {
                    std::mem::transmute::<usize, unsafe extern "C" fn($($kind),*) -> i32>(address)($($argument),*)
                },
                None => OPENCL_UNAVAILABLE,
            }
        }
    };
}

/// Defines a creating function, whose last argument receives the status.
macro_rules! forward_create {
    ($name:ident($($argument:ident: $kind:ty),*; $error:ident)) => {
        /// Replaces the OpenCL library's export of the same name, which ggml links against.
        ///
        /// Inputs: As OpenCL defines them. Outputs: The new object, or null with the status set.
        /// Side effects: Those of the OpenCL function; may load the library.
        ///
        /// # Safety
        /// Follows the OpenCL contract for this function.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name($($argument: $kind,)* $error: *mut i32) -> Pointer {
            static SYMBOL: OnceLock<Option<usize>> = OnceLock::new();
            let symbol = *SYMBOL.get_or_init(|| opencl_symbol(
                // SAFETY: The literal ends in a NUL byte.
                unsafe { CStr::from_bytes_with_nul_unchecked(concat!(stringify!($name), "\0").as_bytes()) }
            ));
            match symbol {
                // SAFETY: The address is this function's, whose signature OpenCL defines as here.
                Some(address) => unsafe {
                    std::mem::transmute::<usize, unsafe extern "C" fn($($kind,)* *mut i32) -> Pointer>(address)($($argument,)* $error)
                },
                None => {
                    if !$error.is_null() {
                        // SAFETY: OpenCL lets the caller pass null or a valid status location.
                        unsafe { *$error = OPENCL_UNAVAILABLE };
                    }
                    std::ptr::null_mut()
                }
            }
        }
    };
}

/// Replaces the OpenCL library's export of the same name, which ggml links against.
///
/// Inputs: As OpenCL defines them. Outputs: OpenCL's status, or `OPENCL_UNAVAILABLE` on a phone without
/// a GPU efficient mode supports, so ggml goes no further with that phone's driver.
/// Side effects: May load the library.
///
/// # Safety
/// Follows the OpenCL contract for this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn clGetPlatformIDs(count: u32, platforms: Pointer, found: *mut u32) -> i32 {
    match (efficient_gpu_version(), opencl_symbol(c"clGetPlatformIDs")) {
        // SAFETY: The address is this function's, whose signature OpenCL defines as here.
        (Some(_), Some(address)) => unsafe {
            std::mem::transmute::<usize, GetPlatformIds>(address)(count, platforms, found)
        },
        _ => OPENCL_UNAVAILABLE,
    }
}

forward_status!(
    clGetPlatformInfo(
        platform: Pointer,
        name: u32,
        size: usize,
        value: Pointer,
        written: *mut usize,
    )
);
forward_status!(
    clGetDeviceIDs(
        platform: Pointer,
        device_type: u64,
        count: u32,
        devices: Pointer,
        found: *mut u32,
    )
);
forward_status!(
    clGetDeviceInfo(device: Pointer, name: u32, size: usize, value: Pointer, written: *mut usize)
);
forward_create!(clCreateContext(properties: Pointer, count: u32, devices: Pointer, callback: Pointer, user_data: Pointer; error));
forward_status!(clReleaseContext(context: Pointer));
forward_create!(clCreateCommandQueue(context: Pointer, device: Pointer, properties: u64; error));
forward_create!(clCreateBuffer(context: Pointer, flags: u64, size: usize, host: Pointer; error));
forward_create!(clCreateSubBuffer(buffer: Pointer, flags: u64, kind: u32, info: Pointer; error));
forward_create!(clCreateImage(context: Pointer, flags: u64, format: Pointer, description: Pointer, host: Pointer; error));
forward_status!(clReleaseMemObject(memory: Pointer));
forward_create!(clCreateProgramWithSource(context: Pointer, count: u32, sources: Pointer, lengths: Pointer; error));
forward_status!(
    clBuildProgram(
        program: Pointer,
        count: u32,
        devices: Pointer,
        options: Pointer,
        callback: Pointer,
        user_data: Pointer,
    )
);
forward_status!(
    clGetProgramBuildInfo(
        program: Pointer,
        device: Pointer,
        name: u32,
        size: usize,
        value: Pointer,
        written: *mut usize,
    )
);
forward_status!(clReleaseProgram(program: Pointer));
forward_create!(clCreateKernel(program: Pointer, name: Pointer; error));
forward_status!(
    clGetKernelWorkGroupInfo(
        kernel: Pointer,
        device: Pointer,
        name: u32,
        size: usize,
        value: Pointer,
        written: *mut usize,
    )
);
forward_status!(clSetKernelArg(kernel: Pointer, index: u32, size: usize, value: Pointer));
forward_status!(
    clEnqueueNDRangeKernel(
        queue: Pointer,
        kernel: Pointer,
        dimensions: u32,
        offset: Pointer,
        global: Pointer,
        local: Pointer,
        waits: u32,
        wait_list: Pointer,
        event: Pointer,
    )
);
forward_status!(
    clEnqueueReadBuffer(
        queue: Pointer,
        buffer: Pointer,
        blocking: u32,
        offset: usize,
        size: usize,
        target: Pointer,
        waits: u32,
        wait_list: Pointer,
        event: Pointer,
    )
);
forward_status!(
    clEnqueueWriteBuffer(
        queue: Pointer,
        buffer: Pointer,
        blocking: u32,
        offset: usize,
        size: usize,
        source: Pointer,
        waits: u32,
        wait_list: Pointer,
        event: Pointer,
    )
);
forward_status!(
    clEnqueueCopyBuffer(
        queue: Pointer,
        source: Pointer,
        target: Pointer,
        source_offset: usize,
        target_offset: usize,
        size: usize,
        waits: u32,
        wait_list: Pointer,
        event: Pointer,
    )
);
forward_status!(
    clEnqueueFillBuffer(
        queue: Pointer,
        buffer: Pointer,
        pattern: Pointer,
        pattern_size: usize,
        offset: usize,
        size: usize,
        waits: u32,
        wait_list: Pointer,
        event: Pointer,
    )
);
forward_status!(
    clEnqueueBarrierWithWaitList(queue: Pointer, waits: u32, wait_list: Pointer, event: Pointer)
);
forward_status!(
    clEnqueueMarkerWithWaitList(queue: Pointer, waits: u32, wait_list: Pointer, event: Pointer)
);
forward_status!(clFinish(queue: Pointer));
forward_status!(clFlush(queue: Pointer));
forward_status!(clWaitForEvents(count: u32, events: Pointer));
forward_status!(clReleaseEvent(event: Pointer));
