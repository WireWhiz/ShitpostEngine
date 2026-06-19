brane_weaver::proc::register_all_modules!("modules/");

pub mod meta_type;
pub mod task;

pub struct Runtime {}

impl Runtime {
    pub fn new() {
        // TODO add in hot-reload client connection
    }
}
