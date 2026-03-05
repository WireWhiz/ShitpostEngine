use std::{
    ffi::{CString, c_char},
    mem, ptr,
    str::FromStr,
};

use thiserror::Error;
pub use vulkan_c::*;

use crate::windowing::Window;

pub struct Graphics {
    pub instance: VkInstance,
    pub gpu: VkPhysicalDevice,
    pub device: VkDevice,
    pub graphics_queue: VkQueue,
    pub surface: Option<VkSurfaceKHR>,
    pub present_queue: Option<VkQueue>,
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

            let mut extensions = Vec::new();

            #[cfg(debug_assertions)]
            {
                extensions.push(c"VK_LAYER_KHRONOS_validation".as_ptr());
            }

            #[cfg(windows)]
            {
                extensions.extend_from_slice(&[
                    VK_KHR_SURFACE_EXTENSION_NAME.as_ptr(),
                    VK_KHR_WIN32_SURFACE_EXTENSION_NAME.as_ptr(),
                ]);
            }

            check_vk(vkCreateInstance(
                &VkInstanceCreateInfo {
                    sType: VkStructureType::VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
                    pApplicationInfo: &VkApplicationInfo {
                        sType: VkStructureType::VK_STRUCTURE_TYPE_APPLICATION_INFO,
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
                    sType: VkStructureType::VK_STRUCTURE_TYPE_WIN32_SURFACE_CREATE_INFO_KHR,
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
                if props.deviceType == VkPhysicalDeviceType::VK_PHYSICAL_DEVICE_TYPE_DISCRETE_GPU {
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

            let mut queue_create_infos = Vec::new();

            let graphics_queue_family_index = queue_familes
                .iter()
                .enumerate()
                .find_map(|(i, f)| {
                    if f.queueFlags & VkQueueFlagBits::VK_QUEUE_GRAPHICS_BIT.0 as u32 > 0 {
                        Some(i)
                    } else {
                        None
                    }
                })
                .ok_or(GraphicsCreateError::MissingRequiredVulkanQueueTypes)?;

            let graphics_queue_priorities = [0.5f32];
            queue_create_infos.push(VkDeviceQueueCreateInfo {
                sType: VkStructureType::VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
                pNext: ptr::null(),
                flags: 0,
                queueFamilyIndex: graphics_queue_family_index as u32,
                queueCount: graphics_queue_priorities.len() as u32,
                pQueuePriorities: graphics_queue_priorities.as_ptr(),
            });

            let present_queue_family_index = if let Some(surface) = surface {
                {
                    let index = queue_familes
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
                        .ok_or(GraphicsCreateError::MissingRequiredVulkanQueueTypes)??;

                    let present_queue_priorties = [0.5f32];
                    queue_create_infos.push(VkDeviceQueueCreateInfo {
                        sType: VkStructureType::VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
                        pNext: ptr::null(),
                        flags: 0,
                        queueFamilyIndex: index as u32,
                        queueCount: present_queue_priorties.len() as u32,
                        pQueuePriorities: present_queue_priorties.as_ptr(),
                    });
                    Some(index)
                }
            } else {
                None
            };

            // Create logical device
            let logical_device_extension_names = [VK_KHR_SWAPCHAIN_EXTENSION_NAME.as_ptr()];

            let mut enabled_features: VkPhysicalDeviceFeatures2 = mem::zeroed();
            enabled_features.sType = VkStructureType::VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2;
            enabled_features.features.multiViewport = VK_TRUE;

            let mut vk13_features: VkPhysicalDeviceVulkan13Features = mem::zeroed();
            vk13_features.sType =
                VkStructureType::VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_3_FEATURES;
            enabled_features.pNext = mem::transmute(&vk13_features);

            let mut eds_features: VkPhysicalDeviceExtendedDynamicStateFeaturesEXT = mem::zeroed();
            eds_features.sType =
                VkStructureType::VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_EXTENDED_DYNAMIC_STATE_FEATURES_EXT;
            vk13_features.pNext = mem::transmute(&eds_features);

            // Create logical device
            let device_create_info = VkDeviceCreateInfo {
                sType: VkStructureType::VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
                pNext: mem::transmute(&enabled_features),
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

            // Fix "never used" warnings
            drop(vk13_features);
            drop(eds_features);

            let mut device = mem::zeroed();
            check_vk(vkCreateDevice(
                gpu,
                &device_create_info,
                ptr::null(),
                &mut device,
            ))?;

            // Retrieve queues from device
            let mut graphics_queue = mem::zeroed();
            vkGetDeviceQueue(
                device,
                graphics_queue_family_index as u32,
                0,
                &mut graphics_queue,
            );

            let present_queue = if let Some(queue_index) = present_queue_family_index {
                let mut queue = mem::zeroed();
                vkGetDeviceQueue(device, queue_index as u32, 0, &mut queue);
                Some(queue)
            } else {
                None
            };

            Ok(Graphics {
                instance,
                gpu,
                graphics_queue,
                device,
                surface,
                present_queue,
            })
        }
    }
}

#[derive(Error, Debug)]
pub enum GraphicsCreateError {
    #[error("Vulkan Error: `{0}`")]
    VkError(#[from] VkError),

    #[error("No compatable gpus or graphics devices found.")]
    NoCompatableVulkanDevice,

    #[error("No vulkan queue families found that meet queue requirements")]
    MissingRequiredVulkanQueueTypes,
}

pub struct Surface {
    pub handle: VkSurfaceKHR,
}
