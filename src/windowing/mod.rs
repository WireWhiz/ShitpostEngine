use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::AtomicBool;

use thiserror::Error;
use windows::Win32::Foundation::{GetLastError, HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GetModuleHandleExW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DispatchMessageW, GWLP_USERDATA, GetMessageW,
    GetWindowLongPtrW, RegisterClassExW, SW_SHOW, SetWindowLongPtrW, ShowWindow, TranslateMessage,
    WINDOW_EX_STYLE, WM_CREATE, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};
use windows::core::w;

pub struct WindowProcState {
    message: String,
}

pub struct Window {
    pub handle: HWND,
    pub hinstance: HINSTANCE,
    window_callback_state: Box<WindowProcState>,
}

static WINDOW_CLASS_CREATED: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn windows_window_proc(
    window_handle: HWND,
    message_code: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let state = GetWindowLongPtrW(window_handle, GWLP_USERDATA) as *mut WindowProcState;
        match message_code {
            WM_CREATE => {
                let state = (*(lparam.0 as *const CREATESTRUCTW)).lpCreateParams;

                SetWindowLongPtrW(window_handle, GWLP_USERDATA, state as isize);
                LRESULT(0)
            }
            _ => {
                if !state.is_null() {
                    /*println!(
                        "Unmapped window event {} for window {}",
                        message_code,
                        (*state).message
                    );*/
                }
                DefWindowProcW(window_handle, message_code, wparam, lparam)
            }
        }
    }
}

#[allow(non_snake_case)]
impl Window {
    pub fn new() -> Result<Window, WindowCreateError> {
        unsafe {
            let hinstance: HINSTANCE = {
                let mut hmodule: HMODULE = HMODULE(ptr::null_mut());
                GetModuleHandleExW(
                    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                    w!("main"),
                    &mut hmodule,
                )
                .unwrap();
                hmodule.into()
            };
            if !WINDOW_CLASS_CREATED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                let wndclass = WNDCLASSEXW {
                    cbSize: (size_of::<WNDCLASSEXW>() as u32),
                    lpfnWndProc: Some(windows_window_proc), //todo!("We need a window procedure"),
                    hInstance: hinstance,
                    lpszClassName: w!("Shitpost Engine Class"),
                    ..Default::default()
                };
                let register_res = RegisterClassExW(&wndclass);
                println!("Class window class id is {}", register_res);
                if register_res == 0 {
                    panic!(
                        "Failed to create window class: {}",
                        GetLastError().to_hresult().message()
                    );
                }
            }

            let window_callback_state = Box::new(WindowProcState {
                message: String::from("Hello window state"),
            });

            let handle = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("Shitpost Engine Class"),
                w!("Shitpost engine"),
                // Change this if we want fancy border in the future
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                1920,
                1080,
                None,
                None,
                Some(hinstance),
                Some(window_callback_state.as_ref() as *const WindowProcState as *const c_void),
            )
            .map_err(|_| WindowCreateError::FailedToCreateWindow)?;

            let _was_shown = ShowWindow(
                handle,
                windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD::default(),
            );

            let _was_shown = ShowWindow(handle, SW_SHOW);

            Ok(Window {
                handle,
                window_callback_state,
                hinstance,
            })
        }
    }

    pub fn update(&mut self) {
        let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
        unsafe {
            while GetMessageW(&mut msg, Some(self.handle), 0, 0).as_bool() {
                let _did_translate = TranslateMessage(&mut msg);
                let _res = DispatchMessageW(&msg);
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum WindowCreateError {
    #[error("Failed to create window")]
    FailedToCreateWindow,
}
