// build.rs
use std::{env, path::PathBuf};

fn main() {
    // Tell cargo to re-run if env changes, bindgen automatically registers wrapper.h
    println!("cargo:rerun-if-env-changed=VULKAN_SDK");

    let target = env::var("TARGET").unwrap();

    let (wrapper, out_name) = if target.contains("windows") {
        println!("cargo:rustc-link-lib=vulkan-1");

        ("src/wrapper_win32.h", "src/vk_win32.rs")
    } else if target.contains("android") {
        println!("cargo:rustc-link-lib=vulkan");

        ("src/wrapper_android.h", "src/vk_android.rs")
    } else {
        println!("cargo:rustc-link-lib=vulkan");

        ("src/wrapper_xlib.h", "src/vk_xlib.rs")
    };

    // Find headers — prefer VULKAN_SDK env var, fall back to system paths
    let vulkan_include = if let Ok(sdk) = env::var("VULKAN_SDK") {
        // Make sure on windows we're searching for the lib in the sdk
        println!("cargo::rustc-link-search={}\\Lib", sdk);

        PathBuf::from(sdk).join("include")
    } else {
        // typical Linux system install
        let default_include = "/usr/include";
        println!(
            "cargo:warning=VULKAN_SDK not set, using default include directory {}",
            default_include
        );
        PathBuf::from(default_include)
    };

    let bindings = bindgen::Builder::default()
        .header(wrapper)
        .clang_arg(format!("-I{}", vulkan_include.display()))
        // Represent enums as constants (matches Vulkan's C style)
        .default_enum_style(bindgen::EnumVariation::Consts)
        .prepend_enum_name(false)
        .translate_enum_integer_types(true)
        // Generate layout tests so you catch ABI mismatches early
        .layout_tests(true)
        // Block everything but vulkan types
        .allowlist_type("Vk.*")
        .allowlist_function("vk.*")
        .allowlist_var("VK_.*")
        .derive_debug(true)
        .use_core()
        .generate_cstr(true)
        // generates the fn pointer struct
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_name)
        .expect("Couldn't write bindings");
}
