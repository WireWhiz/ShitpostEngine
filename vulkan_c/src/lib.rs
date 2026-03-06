#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(unnecessary_transmutes)]

#[cfg(target_os = "windows")]
pub mod vk_win32;
use std::{env, io::Write};

#[cfg(target_os = "windows")]
pub use vk_win32::*;

#[cfg(target_os = "android")]
pub mod vk_android;
#[cfg(target_os = "android")]
pub use vk_android::*;

#[cfg(all(unix, not(target_os = "android")))]
pub mod vk_wayland;
#[cfg(all(unix, not(target_os = "android")))]
pub use vk_wayland::*;

use thiserror::Error;

pub const fn make_api_version(variant: u32, major: u32, minor: u32, patch: u32) -> u32 {
    (variant << 29) | (major << 22) | (minor << 12) | patch
}

// Major Core Versions (Variant 0, Patch 0)
pub const VK_API_VERSION_1_0: u32 = make_api_version(0, 1, 0, 0);
pub const VK_API_VERSION_1_1: u32 = make_api_version(0, 1, 1, 0);
pub const VK_API_VERSION_1_2: u32 = make_api_version(0, 1, 2, 0);
pub const VK_API_VERSION_1_3: u32 = make_api_version(0, 1, 3, 0);
pub const VK_API_VERSION_1_4: u32 = make_api_version(0, 1, 4, 0);

pub fn VK_MAKE_VERSION(major: u32, minor: u32, patch: u32) -> u32 {
    (major << 22u32) | (minor << 12u32) | (patch)
}

pub fn check_vk(error: VkResult) -> Result<(), VkError> {
    if cfg!(debug_assertions) && env::var("RUST_BACKTRACE").unwrap_or("0".into()) != "0" {
        VkError::from_raw(error).expect("Vulkan error");
    } else {
        VkError::from_raw(error)?;
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum VkError {
    // --- Core 1.0 errors ---
    #[error("A host memory allocation has failed")]
    OutOfHostMemory,
    #[error("A device memory allocation has failed")]
    OutOfDeviceMemory,
    #[error(
        "Initialization of an object could not be completed for implementation-specific reasons"
    )]
    InitializationFailed,
    #[error("The logical or physical device has been lost")]
    DeviceLost,
    #[error("Mapping of a memory object has failed")]
    MemoryMapFailed,
    #[error("A requested layer is not present or could not be loaded")]
    LayerNotPresent,
    #[error("A requested extension is not supported")]
    ExtensionNotPresent,
    #[error("A requested feature is not supported")]
    FeatureNotPresent,
    #[error(
        "The requested version of Vulkan is not supported by the driver or is otherwise incompatible for implementation-specific reasons"
    )]
    IncompatibleDriver,
    #[error("Too many objects of the type have already been created")]
    TooManyObjects,
    #[error("A requested format is not supported on this device")]
    FormatNotSupported,
    #[error("A requested pool allocation has failed due to fragmentation of the pool's memory")]
    FragmentedPool,
    #[error(
        "An unknown error has occurred, either the application has provided invalid input, or an implementation failure has occurred"
    )]
    Unknown,

    // --- Core 1.1 errors ---
    #[error("A pool memory allocation has failed")]
    OutOfPoolMemory,
    #[error("An external handle is not a valid handle of the specified type")]
    InvalidExternalHandle,

    // --- Core 1.2 errors ---
    #[error("A descriptor pool creation has failed due to fragmentation")]
    Fragmentation,
    #[error(
        "A buffer creation or memory allocation failed because the requested address is not available"
    )]
    InvalidOpaqueCaptureAddress,

    // --- Core 1.0 validation (EXT) ---
    #[error(
        "A command failed because invalid usage was detected by the implementation or a validation layer"
    )]
    ValidationFailed,

    // --- Surface/swapchain (KHR) ---
    #[error("A surface is no longer available")]
    SurfaceLost,
    #[error(
        "The requested window is already in use by Vulkan or another API in a manner that prevents it from being used again"
    )]
    NativeWindowInUse,
    #[error(
        "A surface has changed in such a way that it is no longer compatible with the swapchain, and further presentation requests using the swapchain will fail"
    )]
    OutOfDate,
    #[error(
        "The display used by a swapchain does not use the same presentable image layout, or is incompatible in a way that prevents sharing an image"
    )]
    IncompatibleDisplay,

    // --- Validation/pipeline cache ---
    #[error("The supplied pipeline cache data was not valid for the current implementation")]
    InvalidPipelineCacheData,
    #[error(
        "The implementation did not find a match in the pipeline cache for the specified pipeline"
    )]
    NoPipelineMatch,

    // --- Full screen exclusive (EXT) ---
    #[error(
        "An operation on a swapchain created with VK_FULL_SCREEN_EXCLUSIVE_APPLICATION_CONTROLLED_EXT failed as it did not have exclusive full-screen access"
    )]
    FullScreenExclusiveModeLost,

    // --- Privileges ---
    #[error(
        "The driver implementation has denied a request to acquire a priority above the default priority because the application does not have sufficient privileges"
    )]
    NotPermitted,

    // --- Space (KHR) ---
    #[error("The application did not provide enough space to return all the required data")]
    NotEnoughSpace,

    // --- Video (KHR) ---
    #[error("The specified video picture layout is not supported")]
    VideoProfileOperationNotSupported,
    #[error("The specified video profile is not supported")]
    VideoProfileFormatNotSupported,
    #[error("The specified video profile is not supported for the specified codec")]
    VideoProfileCodecNotSupported,
    #[error("The specified video Std header version is not supported")]
    VideoStdVersionNotSupported,
    #[error(
        "The specified Video Std parameters do not adhere to the syntactic or semantic requirements of the used video compression standard"
    )]
    InvalidVideoStdParameters,

    // --- Shader binary (EXT) ---
    #[error("The provided binary shader code is not compatible with this device")]
    IncompatibleShaderBinary,

    // --- Catch-all for unknown raw codes ---
    #[error("Unknown VkResult error code: {0}")]
    UnrecognizedCode(i32),
}

impl VkError {
    pub fn from_raw(result: VkResult) -> Result<VkResult, VkError> {
        // Positive/zero values are success or informational — not errors
        if result >= 0 {
            return Ok(result);
        }
        Err(match result {
            -1 => VkError::OutOfHostMemory,
            -2 => VkError::OutOfDeviceMemory,
            -3 => VkError::InitializationFailed,
            -4 => VkError::DeviceLost,
            -5 => VkError::MemoryMapFailed,
            -6 => VkError::LayerNotPresent,
            -7 => VkError::ExtensionNotPresent,
            -8 => VkError::FeatureNotPresent,
            -9 => VkError::IncompatibleDriver,
            -10 => VkError::TooManyObjects,
            -11 => VkError::FormatNotSupported,
            -12 => VkError::FragmentedPool,
            -13 => VkError::Unknown,
            -1000069000 => VkError::OutOfPoolMemory,
            -1000072003 => VkError::InvalidExternalHandle,
            -1000161000 => VkError::Fragmentation,
            -1000257000 => VkError::InvalidOpaqueCaptureAddress,
            -1000011001 => VkError::ValidationFailed,
            -1000000000 => VkError::SurfaceLost,
            -1000000001 => VkError::NativeWindowInUse,
            -1000001004 => VkError::OutOfDate,
            -1000003001 => VkError::IncompatibleDisplay,
            -1000012000 => VkError::InvalidPipelineCacheData,
            -1000012001 => VkError::NoPipelineMatch,
            -1000255000 => VkError::FullScreenExclusiveModeLost,
            -1000174001 => VkError::NotPermitted,
            -1000483000 => VkError::NotEnoughSpace,
            -1000023000 => VkError::VideoProfileOperationNotSupported,
            -1000023001 => VkError::VideoProfileFormatNotSupported,
            -1000023004 => VkError::VideoProfileCodecNotSupported,
            -1000023005 => VkError::VideoStdVersionNotSupported,
            -1000299000 => VkError::InvalidVideoStdParameters,
            -1000482000 => VkError::IncompatibleShaderBinary,
            code => VkError::UnrecognizedCode(code),
        })
    }
}
