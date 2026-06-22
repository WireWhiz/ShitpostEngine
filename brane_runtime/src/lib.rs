use crate::task::TaskGraph;

brane_weaver::proc::register_all_modules!("modules/");

pub use brane_runtime_proc as proc;
pub mod meta_type;
pub mod stable_table;
pub mod task;

pub struct Runtime {
    pub main_graph: TaskGraph,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            main_graph: TaskGraph::new(),
        }
    }
}
