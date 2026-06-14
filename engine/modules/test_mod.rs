use brane_weaver::ModuleHandle;

#[unsafe(no_mangle)]
pub extern "Rust" fn allocate(name: &str) -> Box<dyn ModuleHandle> {
    Box::new(TestModule {
        name: name.to_string(),
    })
}

pub struct TestModule {
    name: String,
}

impl ModuleHandle for TestModule {
    fn name(&self) -> &str {
        &self.name
    }

    fn tick(&mut self) {
        println!("Hot reloading");
    }
}
