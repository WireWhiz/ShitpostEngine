use std::{
    ffi::{CString, c_void},
    hint, mem, ptr,
    str::FromStr,
};

use std::ffi::CStr;
use thiserror::Error;
pub use vulkan_c::*;

use crate::windowing::Window;

pub struct WindowCtx {
    pub surface: VkSurfaceKHR,
    pub present_queue: VkQueue,
    pub swapchain: VkSwapchainKHR,
}

pub struct Graphics {
    pub instance: VkInstance,
    pub gpu: VkPhysicalDevice,
    pub device: VkDevice,
    pub graphics_queue: VkQueue,
    pub window: Option<WindowCtx>,
}

impl Graphics {
    pub fn new(
        application_name: &str,
        window: Option<&Window>,
    ) -> Result<Graphics, GraphicsCreateError> {
        unsafe {
            let mut instance = VkInstance::default();
            let application_name =
                CString::from_str(application_name).expect("Invalid application name");

            let mut aext_count = 0;
            vkEnumerateInstanceExtensionProperties(ptr::null(), &mut aext_count, ptr::null_mut());
            let mut available_extensions = Vec::with_capacity(aext_count as usize);
            vkEnumerateInstanceExtensionProperties(
                ptr::null(),
                &mut aext_count,
                available_extensions.as_mut_ptr(),
            );
            available_extensions.set_len(aext_count as usize);

            let mut extensions = Vec::new();

            #[cfg(debug_assertions)]
            {
                let validation = c"VK_LAYER_KHRONOS_validation".as_ptr();
                if !available_extensions
                    .iter()
                    .find(|layer| {
                        CStr::from_ptr(layer.extensionName.as_ptr()) == CStr::from_ptr(validation)
                    })
                    .is_some()
                {
                    println!(
                        "{} not supported, ignoring.",
                        CStr::from_ptr(validation).to_str().unwrap()
                    );
                } else {
                    extensions.push(validation);
                }
                extensions.push(c"VK_EXT_debug_utils".as_ptr());
            }

            #[cfg(windows)]
            {
                extensions.extend_from_slice(&[
                    VK_KHR_SURFACE_EXTENSION_NAME.as_ptr(),
                    VK_KHR_WIN32_SURFACE_EXTENSION_NAME.as_ptr(),
                ]);
            }

            for e in &extensions {
                if !available_extensions
                    .iter()
                    .find(|layer| {
                        CStr::from_ptr(layer.extensionName.as_ptr()) == CStr::from_ptr(*e)
                    })
                    .is_some()
                {
                    println!(
                        "{} is not a supported instance extension.",
                        CStr::from_ptr(*e).to_str().unwrap()
                    );
                    return Err(GraphicsCreateError::RequredInstanceExtensionWasNotFound(
                        CStr::from_ptr(*e).to_str().unwrap().to_string(),
                    ));
                }
            }

            check_vk(vkCreateInstance(
                &VkInstanceCreateInfo {
                    sType: VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
                    pApplicationInfo: &VkApplicationInfo {
                        sType: VK_STRUCTURE_TYPE_APPLICATION_INFO,
                        pApplicationName: application_name.as_ptr(),
                        applicationVersion: VK_MAKE_VERSION(0, 1, 0),
                        pEngineName: c"Shitpost Engine".as_ptr(),
                        engineVersion: VK_MAKE_VERSION(0, 1, 0),
                        apiVersion: VK_API_VERSION_1_3,
                        pNext: ptr::null(),
                    },
                    enabledExtensionCount: extensions.len() as u32,
                    ppEnabledExtensionNames: extensions.as_ptr(),
                    enabledLayerCount: 0,
                    ppEnabledLayerNames: ptr::null(),
                    pNext: ptr::null(),
                    flags: 0,
                },
                ptr::null(),
                &mut instance,
            ))?;

            println!("Vulkan instance created successfully!");

            let surface = if let Some(window) = window {
                let create_info = VkWin32SurfaceCreateInfoKHR {
                    sType: VK_STRUCTURE_TYPE_WIN32_SURFACE_CREATE_INFO_KHR,
                    pNext: ptr::null(),
                    flags: 0,
                    hinstance: mem::transmute(window.hinstance.0),
                    hwnd: mem::transmute(window.handle.0),
                };
                let mut surface = mem::zeroed();
                check_vk(vkCreateWin32SurfaceKHR(
                    instance,
                    &create_info,
                    ptr::null(),
                    &mut surface,
                ))?;
                println!("Created Vulkan surface for window");
                Some(surface)
            } else {
                None
            };

            // TODO: Don't hardcode these
            let swapchain_fmt = VK_FORMAT_B8G8R8A8_UNORM;
            let swapchain_color_space = VK_COLOR_SPACE_SRGB_NONLINEAR_KHR;

            // Choose device
            let mut device_count = 0u32;
            // Get device count
            check_vk(vkEnumeratePhysicalDevices(
                instance,
                &mut device_count,
                ptr::null_mut(),
            ))?;

            // Get devices
            let mut devices = Vec::with_capacity(device_count as usize);
            devices.set_len(device_count as usize);
            check_vk(vkEnumeratePhysicalDevices(
                instance,
                &mut device_count,
                devices.as_mut_ptr(),
            ))?;

            // Rate devices and select one
            let mut best_device = None;
            let mut best_score = 0;
            for device in devices {
                let mut features: VkPhysicalDeviceFeatures = mem::zeroed();
                vkGetPhysicalDeviceFeatures(device, &mut features);
                let mut props: VkPhysicalDeviceProperties = mem::zeroed();
                vkGetPhysicalDeviceProperties(device, &mut props);

                let mut score = 1;

                // If this is an actual graphics card we want it
                if props.deviceType == VK_PHYSICAL_DEVICE_TYPE_DISCRETE_GPU {
                    score += 10000;
                }

                if score > best_score {
                    best_device = Some(device);
                    best_score = score;
                }
            }

            let Some(gpu) = best_device else {
                return Err(GraphicsCreateError::NoCompatableVulkanDevice);
            };

            let mut props: VkPhysicalDeviceProperties = mem::zeroed();
            vkGetPhysicalDeviceProperties(gpu, &mut props);
            println!(
                "Selected physical device: {}",
                str::from_utf8_unchecked(mem::transmute(props.deviceName.as_slice()))
            );

            // Create graphics queues
            let mut queue_family_count = 0u32;
            vkGetPhysicalDeviceQueueFamilyProperties(gpu, &mut queue_family_count, ptr::null_mut());

            let mut queue_familes = Vec::with_capacity(queue_family_count as usize);
            vkGetPhysicalDeviceQueueFamilyProperties(
                gpu,
                &mut queue_family_count,
                queue_familes.as_mut_ptr(),
            );
            queue_familes.set_len(queue_family_count as usize);

            let mut queue_create_infos = Vec::new();

            // We're not setting priorities right now, so account for max number and reuse
            let queue_priorities = [0.5f32, 0.5f32, 0.5f32, 0.5f32];

            let graphics_queue_family_index = queue_familes
                .iter()
                .enumerate()
                .find_map(|(i, f)| {
                    if (f.queueFlags & VK_QUEUE_GRAPHICS_BIT as u32) > 0 {
                        Some(i)
                    } else {
                        None
                    }
                })
                .ok_or(GraphicsCreateError::MissingRequiredVulkanQueueType(
                    "VK_QUEUE_GRAPHICS_BIT",
                ))?;

            queue_create_infos.push(VkDeviceQueueCreateInfo {
                sType: VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
                pNext: ptr::null(),
                flags: 0,
                queueFamilyIndex: graphics_queue_family_index as u32,
                queueCount: 1,
                pQueuePriorities: queue_priorities.as_ptr(),
            });

            // Try to find a dedicated transfer queue, otherwise use any transfer queue
            let transfer_queue_family_index = queue_familes
                .iter()
                .enumerate()
                .find_map(|(i, f)| {
                    let supports_tranfers = (f.queueFlags as i32 & VK_QUEUE_TRANSFER_BIT) > 0;
                    let has_other_functions =
                        (f.queueFlags as i32 & (VK_QUEUE_COMPUTE_BIT | VK_QUEUE_GRAPHICS_BIT)) > 0;
                    if supports_tranfers && !has_other_functions {
                        Some(Ok(i))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    queue_familes
                        .iter()
                        .enumerate()
                        .find_map(|(i, f)| {
                            let supports_tranfers =
                                (f.queueFlags as i32 & VK_QUEUE_TRANSFER_BIT) > 0;
                            if supports_tranfers { Some(i) } else { None }
                        })
                        .ok_or(GraphicsCreateError::MissingRequiredVulkanQueueType(
                            "VK_QUEUE_TRANSFER_BIT",
                        ))
                })?;

            let mut transfer_queue_index = 0;
            if transfer_queue_family_index != graphics_queue_family_index {
                queue_create_infos.push(VkDeviceQueueCreateInfo {
                    sType: VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
                    pNext: ptr::null(),
                    flags: 0,
                    queueFamilyIndex: transfer_queue_family_index as u32,
                    queueCount: 1,
                    pQueuePriorities: queue_priorities.as_ptr(),
                });
            } else {
                transfer_queue_index += 1;
                queue_create_infos[0].queueCount += 1;
            }

            let present_queue_indicies = if let Some(surface) = surface {
                {
                    let family_index = queue_familes
                        .iter()
                        .enumerate()
                        .find_map(|(i, f)| {
                            let mut is_supported = VK_FALSE;
                            if let Err(err) = check_vk(vkGetPhysicalDeviceSurfaceSupportKHR(
                                gpu,
                                i as u32,
                                surface,
                                &mut is_supported,
                            )) {
                                return Some(Err(err));
                            }
                            if is_supported == VK_TRUE {
                                Some(Ok(i))
                            } else {
                                None
                            }
                        })
                        .ok_or(GraphicsCreateError::MissingRequiredVulkanQueueType(
                            "present queue",
                        ))??;

                    let queue_index = if family_index == graphics_queue_family_index {
                        let queue_index = queue_create_infos[0].queueCount;
                        queue_create_infos[0].queueCount += 1;
                        queue_index
                    } else if family_index == transfer_queue_family_index {
                        let queue_index = queue_create_infos[1].queueCount;
                        queue_create_infos[1].queueCount += 1;
                        queue_index
                    } else {
                        queue_create_infos.push(VkDeviceQueueCreateInfo {
                            sType: VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
                            pNext: ptr::null(),
                            flags: 0,
                            queueFamilyIndex: family_index as u32,
                            queueCount: 1,
                            pQueuePriorities: queue_priorities.as_ptr(),
                        });
                        0
                    };
                    Some((family_index, queue_index))
                }
            } else {
                None
            };

            // Create logical device
            let logical_device_extension_names = [VK_KHR_SWAPCHAIN_EXTENSION_NAME.as_ptr()];

            let mut enabled_features: VkPhysicalDeviceFeatures2 = mem::zeroed();
            enabled_features.sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2;
            //enabled_features.features.multiViewport = VK_TRUE;

            let mut vk13_features: VkPhysicalDeviceVulkan13Features = mem::zeroed();
            vk13_features.sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_3_FEATURES;
            vk13_features.dynamicRendering = VK_TRUE;

            let mut eds_features: VkPhysicalDeviceExtendedDynamicStateFeaturesEXT = mem::zeroed();
            eds_features.sType =
                VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_EXTENDED_DYNAMIC_STATE_FEATURES_EXT;
            eds_features.extendedDynamicState = VK_TRUE;

            enabled_features.pNext = hint::black_box(&mut vk13_features as *mut _ as *mut c_void);
            vk13_features.pNext = hint::black_box(&mut eds_features as *mut _ as *mut c_void);
            eds_features.pNext = ptr::null_mut();

            println!(
                "Queue family indicies: graphics {}, transfer {}, present {:?}",
                graphics_queue_family_index, transfer_queue_family_index, present_queue_indicies
            );

            // Create logical device
            let device_create_info = VkDeviceCreateInfo {
                sType: VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
                pNext: &mut enabled_features as *mut _ as *mut c_void,
                flags: 0,
                queueCreateInfoCount: queue_create_infos.len() as u32,
                pQueueCreateInfos: queue_create_infos.as_ptr(),
                // Modern vulkan only uses instance layers
                enabledLayerCount: 0,
                ppEnabledLayerNames: ptr::null(),
                enabledExtensionCount: logical_device_extension_names.len() as u32,
                ppEnabledExtensionNames: logical_device_extension_names.as_ptr(),
                // Handled in pNext instead
                pEnabledFeatures: ptr::null(),
            };

            let mut device = mem::zeroed();
            check_vk(vkCreateDevice(
                gpu,
                &device_create_info,
                ptr::null(),
                &mut device,
            ))?;

            // Fix "not used" warning and possible optimizations
            hint::black_box(enabled_features);
            hint::black_box(vk13_features);
            hint::black_box(eds_features);

            println!("Logical vulkan device created");

            // Retrieve queues from device
            let mut graphics_queue = mem::zeroed();
            vkGetDeviceQueue(
                device,
                graphics_queue_family_index as u32,
                0,
                &mut graphics_queue,
            );

            let mut transfer_queue = mem::zeroed();
            vkGetDeviceQueue(
                device,
                transfer_queue_family_index as u32,
                transfer_queue_index,
                &mut transfer_queue,
            );

            let window =
                if let (Some((queue_family_index, queue_index)), Some(surface), Some(window)) =
                    (present_queue_indicies, surface, window)
                {
                    let mut present_queue = mem::zeroed();
                    vkGetDeviceQueue(
                        device,
                        queue_family_index as u32,
                        queue_index,
                        &mut present_queue,
                    );

                    let mut surface_caps = mem::zeroed();
                    vkGetPhysicalDeviceSurfaceCapabilitiesKHR(gpu, surface, &mut surface_caps);

                    let size = window.resolution();

                    let swapchain_create_info = VkSwapchainCreateInfoKHR {
                        sType: VK_STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR,
                        pNext: ptr::null(),
                        flags: 0,
                        surface,
                        minImageCount: surface_caps.minImageCount + 1,
                        imageFormat: swapchain_fmt,
                        imageColorSpace: swapchain_color_space,
                        imageExtent: VkExtent2D {
                            width: size.x as u32,
                            height: size.y as u32,
                        },
                        imageArrayLayers: 1,
                        imageUsage: VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT as u32,
                        imageSharingMode: VK_SHARING_MODE_EXCLUSIVE,
                        // We only have to set queue families if the mode is not exclusive
                        queueFamilyIndexCount: 0,
                        pQueueFamilyIndices: ptr::null(),
                        preTransform: surface_caps.currentTransform,
                        compositeAlpha: VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR,
                        // Vsync option
                        presentMode: VK_PRESENT_MODE_IMMEDIATE_KHR,
                        clipped: VK_TRUE,
                        oldSwapchain: ptr::null_mut(),
                    };

                    let mut swapchain = mem::zeroed();
                    check_vk(vkCreateSwapchainKHR(
                        device,
                        &swapchain_create_info,
                        ptr::null(),
                        &mut swapchain,
                    ))?;
                    println!("created swapchain!");

                    Some(WindowCtx {
                        surface,
                        present_queue,
                        swapchain,
                    })
                } else {
                    None
                };

            Ok(Graphics {
                instance,
                gpu,
                device,
                window,
                graphics_queue,
            })
        }
    }
}

#[derive(Error, Debug)]
pub enum GraphicsCreateError {
    #[error("Vulkan Error: `{0}`")]
    VkError(#[from] VkError),

    #[error("Required instance extension was not found: \"{0}\"")]
    RequredInstanceExtensionWasNotFound(String),

    #[error("No compatable gpus or graphics devices found.")]
    NoCompatableVulkanDevice,

    #[error("No vulkan queue families found that contain: {0}")]
    MissingRequiredVulkanQueueType(&'static str),
}

pub struct Surface {
    pub handle: VkSurfaceKHR,
}
