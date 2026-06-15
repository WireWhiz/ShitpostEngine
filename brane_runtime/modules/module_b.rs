use brane_weaver::ModuleHandle;

#[unsafe(no_mangle)]
pub extern "C" fn allocate(id: usize) -> Box<dyn ModuleHandle> {}
