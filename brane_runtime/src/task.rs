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
use thiserror::Error;

use crate::{
    define_stable_key,
    meta_type::{MetaTypeDefinition, MetaValueStore, MetaValueVec, StoreValueId},
    stable_table::StableTable,
};

define_stable_key!(TaskHandle);
#[derive(Clone, Copy)]
struct TaskInstancePath {
    pub stage: usize,
    pub instance: usize,
}

pub struct TaskGraph {
    task_instances: StableTable<TaskHandle, TaskInstance>,
    stages: Vec<TaskGraphStage>,
    store: MetaValueStore,
}

struct TaskGraphStage {
    pub tasks: Vec<TaskHandle>,
}

impl TaskGraph {
    pub fn new() -> Self {
        Self {
            task_instances: StableTable::new(),
            stages: Vec::new(),
            store: MetaValueStore::new(),
        }
    }

    pub fn run(&mut self) {
        let mut arg_data = Vec::new();
        arg_data.reserve_exact(16);
        let queues = [];
        for stage in &mut self.stages {
            // TODO multithread this, because it's guarenteed frame safe
            for task in &mut stage.tasks {
                let instance = &self.task_instances[*task];
                let def = instance.def.clone();

                for arg in &instance.args {
                    let task = self.task_instances.get(arg.result_of).unwrap();
                    let value_id = if arg.last_frame {
                        task.last_frame_data
                            .expect("Expected previous frame of data but it was not cached")
                    } else {
                        task.data
                    };
                    arg_data.push(self.store.get(value_id));
                }

                let ret_val = unsafe { self.store.get_mut_unchecked(instance.data) };

                unsafe {
                    (def.callback)(arg_data.as_slice(), &queues, ret_val);
                }

                arg_data.clear();
            }
        }
    }

    // Prefer localized sorting functions, once they exist
    pub fn sort(&mut self) -> Result<(), TaskGraphSortError> {
        let mut placed_instance_stages = HashMap::new();

        for (task_handle, _task) in &self.task_instances {
            Self::resolve_node_placement(
                task_handle,
                &self.task_instances,
                &mut placed_instance_stages,
            )?;
        }

        Ok(())
    }

    fn resolve_node_placement(
        task: TaskHandle,
        tasks: &StableTable<TaskHandle, TaskInstance>,
        placed_instance_stages: &mut HashMap<TaskHandle, usize>,
    ) -> Result<usize, TaskGraphSortError> {
        struct StackFrame {
            task: TaskHandle,
            arg_index: usize,
        }
        let mut stack: Vec<StackFrame> = vec![StackFrame { task, arg_index: 0 }];
        let mut visiting: HashMap<TaskHandle, usize> = HashMap::new(); // node -> stack depth at insertion

        while let Some(StackFrame { task, arg_index }) = stack.last_mut() {
            let task = *task;
            let instance = &tasks[task];

            if placed_instance_stages.contains_key(&task) {
                visiting.remove(&task);
                stack.pop();
                continue;
            }

            if instance.args.is_empty() && instance.queues.is_empty() {
                placed_instance_stages.insert(task, 0);
                visiting.remove(&task);
                stack.pop();
                continue;
            }

            let next_unresolved = instance.args[*arg_index..]
                .iter()
                .enumerate()
                .find(|(_, arg)| !placed_instance_stages.contains_key(&arg.result_of));

            if let Some((offset, arg)) = next_unresolved {
                let dep_task = arg.result_of;
                *arg_index += offset + 1;

                if let Some(&cycle_depth) = visiting.get(&dep_task) {
                    // Reconstruct the cycle by slicing the stack from cycle_depth onwards.
                    let mut cycle: Vec<TaskHandle> = stack[cycle_depth..]
                        .iter()
                        .map(|stack| stack.task)
                        .collect();
                    cycle.push(dep_task); // close the loop
                    return Err(TaskGraphSortError::CycleDetected { cycle });
                }

                let depth = stack.len();
                visiting.insert(dep_task, depth);
                stack.push(StackFrame {
                    task: dep_task,
                    arg_index: 0,
                });
            } else {
                let latest_dep_stage = instance
                    .args
                    .iter()
                    .map(|arg| placed_instance_stages[&arg.result_of])
                    .max()
                    .unwrap_or(0);

                let placement = latest_dep_stage + 1;
                placed_instance_stages.insert(task, placement);
                visiting.remove(&task);
                stack.pop();
            }
        }

        placed_instance_stages
            .get(&task)
            .copied()
            .ok_or_else(|| TaskGraphSortError::CycleDetected { cycle: vec![task] })
    }
}

#[derive(Debug, Error)]
pub enum TaskGraphSortError {
    #[error("Cycle detected in task graph: {}", format_cycle(.cycle))]
    CycleDetected { cycle: Vec<TaskHandle> },
}

// TODO actually print out task names somewhere
fn format_cycle(cycle: &[TaskHandle]) -> String {
    cycle
        .iter()
        .map(|n| n.0.index.to_string())
        .collect::<Vec<_>>()
        .join(" -> ")
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

/// A group of the same task to be run within one graph stage

struct TaskInstance {
    pub def: Arc<TaskDefinition>,
    pub args: Vec<TaskDataArg>,
    pub queues: Vec<TaskQueueArg>,
    pub data: StoreValueId,
    // Stage this task is currently placed in. Do not use during sorting/placement
    pub stage: usize,
    pub last_frame_data: Option<StoreValueId>,
}

pub struct TaskDataArg {
    pub result_of: TaskHandle,
    pub last_frame: bool,
    /// If accessing a member of the value, raw offset so it can be a nested member
    pub byte_offset: usize,
    pub byte_size: usize,
}

pub struct TaskQueueArg {
    pub queue_id: (),
    /// If accessing a member of the value, raw offset so it can be a nested member
    pub byte_offset: usize,
    pub byte_size: usize,
}
