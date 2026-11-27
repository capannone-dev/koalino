//! Decides how local inference uses this machine: which cores, whether the CPU is supported, and
//! which GPU. The decisions are plain functions over measured values, so they can be tested without
//! the hardware; only `preferred_inference_threads` and `missing_cpu_instructions` read the machine.

/// A core counts as fast at this share of the fastest core's top speed.
///
/// An S23 has 3.36, 2.8 and 2.0 GHz cores: its 2.0 GHz cores fall below this and are left out,
/// because whisper.cpp splits work evenly and so waits for the slowest thread.
const FAST_CORE_SPEED_RATIO: f64 = 0.7;

/// Whisper stops gaining from threads about here; see the figures in `DECISION_LOG.md` (#70).
const MAX_INFERENCE_THREADS: usize = 8;

/// A GPU needs room for the model and its working buffers, about half the model's size again.
const GPU_WORKING_MEMORY_NUMERATOR: u64 = 3;
/// See `GPU_WORKING_MEMORY_NUMERATOR`.
const GPU_WORKING_MEMORY_DENOMINATOR: u64 = 2;
/// Kept free on the GPU for the desktop and other apps.
const GPU_RESERVED_BYTES: u64 = 256 * 1024 * 1024;

/// The library backend that reports a GPU. A phone built with both lists its GPU once per backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GpuBackend {
    /// Vulkan: desktop GPUs, and phone GPUs other than Adreno (#184).
    Vulkan,
    /// OpenCL: efficient mode on Adreno phones (#75).
    OpenCl,
    /// Any other backend.
    Other,
}

impl GpuBackend {
    /// Reads the backend's name as ggml reports it, such as `Vulkan` or `OpenCL`.
    ///
    /// Inputs: The registry name. Outputs: The backend, `Other` when not one of ours.
    /// Side effects: None.
    pub fn from_ggml_name(name: &str) -> Self {
        match name {
            "Vulkan" => Self::Vulkan,
            "OpenCL" => Self::OpenCl,
            _ => Self::Other,
        }
    }
}

/// A GPU that the inference library reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuDevice {
    /// Position among the library's GPUs, as whisper.cpp's `gpu_device` counts them.
    pub index: i32,
    /// The backend that reports it.
    pub backend: GpuBackend,
    /// True for a GPU with its own memory; false for one sharing the system's.
    pub is_discrete: bool,
    /// Device memory in bytes. For an integrated GPU this is shared system memory.
    pub total_memory_bytes: u64,
    /// Device memory currently free, in bytes.
    pub free_memory_bytes: u64,
}

/// Counts the cores worth using for inference.
///
/// Inputs: Each core's top speed in any one unit; an empty slice means the speeds are unknown.
/// Outputs: How many cores reach `FAST_CORE_SPEED_RATIO` of the fastest, or `None` when unknown.
/// Side effects: None.
pub fn fast_core_count(core_max_speeds: &[u64]) -> Option<usize> {
    let fastest = *core_max_speeds.iter().max()?;
    let threshold = fastest as f64 * FAST_CORE_SPEED_RATIO;
    Some(
        core_max_speeds
            .iter()
            .filter(|speed| **speed as f64 >= threshold)
            .count(),
    )
}

/// Chooses the inference thread count from what is known about the cores.
///
/// Inputs: The usable logical cores, and the count from `fast_core_count` when known.
/// Outputs: A count from one to `MAX_INFERENCE_THREADS`, never above the usable cores.
/// Side effects: None.
pub fn inference_thread_count(usable_cores: usize, fast_cores: Option<usize>) -> usize {
    fast_cores
        .unwrap_or(usable_cores)
        .min(usable_cores)
        .clamp(1, MAX_INFERENCE_THREADS)
}

/// Returns the thread count for inference on this machine.
///
/// Outputs: The count; it falls back to the usable cores when speeds can't be read (Windows, or a
/// phone that hides them). Side effects: Reads CPU information from `/sys` on Linux and Android.
pub fn preferred_inference_threads() -> usize {
    let usable_cores = std::thread::available_parallelism().map_or(1, usize::from);
    inference_thread_count(usable_cores, fast_core_count(&core_max_speeds()))
}

/// Reads each core's top speed from the kernel.
///
/// Outputs: The speeds in kHz, in core order, or nothing when any are unreadable.
/// Side effects: Reads files in `/sys`.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn core_max_speeds() -> Vec<u64> {
    let Ok(entries) = std::fs::read_dir("/sys/devices/system/cpu") else {
        return Vec::new();
    };
    let mut core_numbers: Vec<u32> = entries
        .flatten()
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()?
                .strip_prefix("cpu")?
                .parse()
                .ok()
        })
        .collect();
    core_numbers.sort_unstable();
    let speeds: Vec<u64> = core_numbers
        .iter()
        .map_while(|core| {
            std::fs::read_to_string(format!(
                "/sys/devices/system/cpu/cpu{core}/cpufreq/cpuinfo_max_freq"
            ))
            .ok()?
            .trim()
            .parse()
            .ok()
        })
        .collect();
    if speeds.len() == core_numbers.len() {
        speeds
    } else {
        Vec::new()
    }
}

/// Reports no speeds where the system has no simple way to read them.
///
/// Outputs: An empty list. Side effects: None.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn core_max_speeds() -> Vec<u64> {
    Vec::new()
}

/// Names the CPU instructions our whisper.cpp build needs and this CPU lacks.
///
/// Outputs: The missing instruction names, empty on a supported CPU. Running whisper.cpp without
/// them would crash the app, so the engine refuses to start instead. Side effects: None.
pub fn missing_cpu_instructions() -> Vec<&'static str> {
    let mut missing = Vec::new();
    #[cfg(target_arch = "x86_64")]
    {
        if !std::arch::is_x86_feature_detected!("avx2") {
            missing.push("AVX2");
        }
        if !std::arch::is_x86_feature_detected!("fma") {
            missing.push("FMA");
        }
        if !std::arch::is_x86_feature_detected!("f16c") {
            missing.push("F16C");
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if !std::arch::is_aarch64_feature_detected!("dotprod") {
            missing.push("dotprod");
        }
        if !std::arch::is_aarch64_feature_detected!("fp16") {
            missing.push("fp16");
        }
    }
    missing
}

/// Chooses the GPU for inference.
///
/// Inputs: The GPUs found, in the library's order.
/// Outputs: The discrete GPU with the most memory, else the first integrated one, else `None`.
/// Side effects: None.
pub fn choose_gpu(devices: &[GpuDevice]) -> Option<GpuDevice> {
    devices
        .iter()
        .filter(|device| device.is_discrete)
        .max_by_key(|device| device.total_memory_bytes)
        .or_else(|| devices.first())
        .copied()
}

/// Finds the first GPU that `backend` reports.
///
/// Inputs: The GPUs found, in the library's order, and the backend wanted.
/// Outputs: That GPU, or `None` when the backend found none. Side effects: None.
pub fn first_gpu_on(devices: &[GpuDevice], backend: GpuBackend) -> Option<GpuDevice> {
    devices
        .iter()
        .find(|device| device.backend == backend)
        .copied()
}

/// Says whether a model is likely to fit on a GPU.
///
/// Inputs: The model file's size and the GPU's free memory, in bytes.
/// Outputs: True when the model, its working memory and a reserve all fit. Side effects: None.
pub fn model_fits_on_gpu(model_bytes: u64, free_memory_bytes: u64) -> bool {
    let needed = model_bytes
        .saturating_mul(GPU_WORKING_MEMORY_NUMERATOR)
        .div_euclid(GPU_WORKING_MEMORY_DENOMINATOR)
        .saturating_add(GPU_RESERVED_BYTES);
    needed <= free_memory_bytes
}

/// Gives the GPU memory to advertise to paired devices.
///
/// Inputs: The chosen GPU, if any.
/// Outputs: A discrete GPU's memory in megabytes. Zero otherwise: an integrated GPU's figure is
/// shared system memory, and would outrank a real GPU in routing. Side effects: None.
pub fn advertised_gpu_megabytes(chosen_gpu: Option<GpuDevice>) -> u32 {
    chosen_gpu
        .filter(|device| device.is_discrete)
        .map_or(0, |device| {
            u32::try_from(device.total_memory_bytes / (1024 * 1024)).unwrap_or(u32::MAX)
        })
}

/// Picks the cores that feed a phone GPU, which work best together on one cluster (#75).
///
/// Inputs: Each core's top speed, in core order; empty when unknown.
/// Outputs: The fastest group of at least two equal cores, because a lone prime core can go offline.
/// Empty when speeds are unknown or all cores are alike, as there is nothing to gain.
/// Side effects: None.
pub fn cores_for_gpu_submission(core_max_speeds: &[u64]) -> Vec<usize> {
    let cores_at = |speed: u64| -> Vec<usize> {
        (0..core_max_speeds.len())
            .filter(|&core| core_max_speeds[core] == speed)
            .collect()
    };
    let mut speeds = core_max_speeds.to_vec();
    speeds.sort_unstable_by(|left, right| right.cmp(left));
    speeds.dedup();
    if speeds.len() < 2 {
        return Vec::new();
    }
    speeds
        .into_iter()
        .map(cores_at)
        .find(|cores| cores.len() >= 2)
        .unwrap_or_default()
}

/// Says whether efficient mode supports a GPU.
///
/// Inputs: The version text OpenCL reports, which names the generation.
/// Outputs: True for the Adreno generations whisper.cpp has tuned kernels for; only the Adreno 740
/// has been tested. Side effects: None.
pub fn is_efficient_gpu(device_version: &str) -> bool {
    device_version.contains("Adreno")
        && ["730", "740", "750", "830"]
            .iter()
            .any(|generation| device_version.contains(generation))
}

/// Shortens a supported GPU's version text for Settings.
///
/// Inputs: Text such as `OpenCL 3.0 Adreno(TM) 740`. Outputs: `Adreno 740`. Side effects: None.
pub fn efficient_gpu_label(device_version: &str) -> String {
    let start = device_version.find("Adreno").unwrap_or(0);
    device_version[start..].replace("(TM)", "")
}
