//! Tasks and the TaskGraph are what's responsable for all execution
//!
//! To make something run, you define a Task, and then put it in a graph.
//!
//! Tasks should be pure functions, they take their input, and generate an ouptut.
//! They do not read or mutate global variables, though similar functionality can
//! be achieved when needed.
//!
//! Task Graphs may repeat, nodes in a repeating task graph can request their own
//! output as input, thus preserving state.
//!
//! Task Graphs are responsible for storing/passing all data sent and read by tasks
//! this is so that we can make sure that there is never any chance of a race condtion
//! when evaluating a graph on multiple threads
//!
//! Graph nodes are organized into "stages" of execution. All nodes in a stage are able to execute
//! in parellel, and all of their output data is stored contiguously.
//!
//! Nodes may request two types of data. Node outputs, or queues. Nodes may produce one struct of
//! data and emit to as many queues as they want. They must declare ahead of time all types of data
//! they might consume or produce

use std::{collections::HashMap, sync::Arc};

use crate::meta_type::{MetaTypeDefinition, MetaValueVec};

#[derive(PartialEq, PartialOrd, Hash, Clone, Copy)]
pub struct TaskHandle(usize);

#[derive(Clone, Copy)]
struct TaskInstancePath {
    pub stage: usize,
    pub instance: usize,
}

pub struct TaskGraph {
    tasks: HashMap<TaskHandle, TaskInstancePath>,
    stages: Vec<TaskGraphStage>,
}

struct TaskGraphStage {
    pub tasks: Vec<TaskInstance>,
}

impl TaskGraph {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            stages: Vec::new(),
        }
    }
}

pub type TaskCallback = unsafe fn(args: &[&[u8]], queues: &[&mut ()], output: &mut [u8]);

struct TaskDefinition {
    pub queue_params: Vec<TaskQueueParam>,
    pub data_params: Vec<TaskDataParam>,
    pub output: TaskOutput,
    pub callback: TaskCallback,
}

struct TaskDataParam {
    pub type_def: &'static MetaTypeDefinition,
}

struct TaskQueueParam {
    pub consumer: bool,
    pub type_def: &'static MetaTypeDefinition,
}

struct TaskOutput {
    pub type_def: &'static MetaTypeDefinition,
}

struct TaskData {
    /// Data from this frame/evaluation
    pub fresh_data: MetaValueVec,
    /// Data from the last frame/evaluation
    pub stale_data: MetaValueVec,
}

struct TaskDataSource {
    pub ,
}

/// A group of the same task to be run within one graph stage
struct TaskGroup {
    /// The definition for this task
}

struct TaskInstance {
    pub def: Arc<TaskDefinition>,
    pub args: Vec<>,
}
