use std::{mem::ManuallyDrop, thread::JoinHandle};

#[path = "server_api.rs"]
pub mod server_api;
use server_api::WeaverMessage;
use websocket::OwnedMessage;

enum Module {
    Static(Box<dyn ModuleHandle>),
    Dynamic {
        lib_handle: ManuallyDrop<libloading::Library>,
        handle: ManuallyDrop<Box<dyn ModuleHandle>>,
    },
}

impl Module {
    fn handle(&mut self) -> &mut dyn ModuleHandle {
        match self {
            Module::Static(h) => h.as_mut(),
            Module::Dynamic {
                handle,
                lib_handle: _,
            } => handle.as_mut(),
        }
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        if let Module::Dynamic { lib_handle, handle } = self {
            unsafe {
                std::mem::ManuallyDrop::drop(handle);
                std::mem::ManuallyDrop::drop(lib_handle);
            }
        }
    }
}

pub struct WeaverClient {
    modules: Vec<Module>,
    update_server_thread: JoinHandle<()>,
    message_queue: std::sync::mpsc::Receiver<WeaverMessage>,
}

/// All modules must define a `pub fn allocate() -> Box<dyn ModuleHandle>` function
pub trait ModuleHandle {
    fn name(&self) -> &str;

    fn tick(&mut self);
}

impl WeaverClient {
    pub fn new(modules: Vec<Box<dyn ModuleHandle>>) -> Self {
        let mut update_server = websocket::ClientBuilder::new("wss://localhost:2001")
            .unwrap()
            .connect_insecure()
            .unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let update_server_thread = std::thread::spawn(move || {
            while let Ok(msg) = update_server.recv_message() {
                if !msg.is_data() {
                    continue;
                }

                if msg.is_close() {
                    println!("Weaver update server closed connection");
                    return;
                }
                if let OwnedMessage::Ping(data) = msg {
                    let _ = update_server.send_message(&OwnedMessage::Pong(data));
                    continue;
                };

                let OwnedMessage::Text(json) = msg else {
                    continue;
                };

                let msg: WeaverMessage = serde_json::from_str(&json).unwrap();
                tx.send(msg).expect("Failed to queue message");
            }
        });
        Self {
            modules: modules.into_iter().map(|m| Module::Static(m)).collect(),
            update_server_thread,
            message_queue: rx,
        }
    }

    pub fn update_modules(&mut self) {
        while let Ok(msg) = self.message_queue.try_recv() {
            match msg {
                WeaverMessage::ReloadModule {
                    module_name,
                    dynamic_lib_path,
                } => {
                    let dylib = unsafe { libloading::Library::new(&dynamic_lib_path) };
                    let dylib = match dylib {
                        Ok(dylib) => dylib,
                        Err(err) => {
                            println!(
                                "Failed to load {} from path {}: {:?}",
                                module_name,
                                dynamic_lib_path,
                                match err {
                                    libloading::Error::DlOpen { source } => todo!(),
                                    libloading::Error::DlOpenUnknown => todo!(),
                                    libloading::Error::DlSym { source } => todo!(),
                                    libloading::Error::DlSymUnknown => todo!(),
                                    libloading::Error::DlClose { source } => todo!(),
                                    libloading::Error::DlCloseUnknown => todo!(),
                                    libloading::Error::LoadLibraryExW { source } =>
                                        panic!("failed to load with windows error {}", source),
                                    libloading::Error::LoadLibraryExWUnknown => todo!(),
                                    libloading::Error::GetModuleHandleExW { source } => todo!(),
                                    libloading::Error::GetModuleHandleExWUnknown => todo!(),
                                    libloading::Error::GetProcAddress { source } => todo!(),
                                    libloading::Error::GetProcAddressUnknown => todo!(),
                                    libloading::Error::FreeLibrary { source } => todo!(),
                                    libloading::Error::FreeLibraryUnknown => todo!(),
                                    libloading::Error::IncompatibleSize => todo!(),
                                    libloading::Error::InteriorZeroElements => todo!(),
                                    _ => todo!(),
                                }
                            );
                            continue;
                        }
                    };

                    let allocate_fn: libloading::Symbol<
                        unsafe extern "Rust" fn(name: &str) -> Box<dyn ModuleHandle>,
                    > = match unsafe { dylib.get(b"allocate") } {
                        Ok(func) => func,
                        Err(err) => {
                            println!("Failed to load allocate() from {}: {:?}", module_name, err);
                            continue;
                        }
                    };

                    self.modules
                        .retain_mut(|m| m.handle().name() != module_name);

                    let reloaded_module = unsafe { allocate_fn(&module_name) };
                    self.modules.push(Module::Dynamic {
                        lib_handle: ManuallyDrop::new(dylib),
                        handle: ManuallyDrop::new(reloaded_module),
                    });
                }
            }
        }
    }

    pub fn get_module_by_name(&mut self, name: &str) -> Option<&mut dyn ModuleHandle> {
        for module in &mut self.modules {
            let module = module.handle();
            if module.name() == name {
                return Some(module);
            } else {
                println!("checked module: {}", module.name());
            }
        }
        None
    }
}
