use brane_runtime::task::{TaskDataArg, TaskDataParam, TaskDefinition};
use brane_runtime::{meta_type::MetaType, task::TaskOutput};
use std::sync::Arc;

fn main() {
    let mut runtime = brane_runtime::Runtime::new();

    let const_zero = runtime.main_graph.store.add(0f32);
    let const_a = runtime.main_graph.store.add(2f32);
    let const_b = runtime.main_graph.store.add(5f32);
    let const_c = runtime.main_graph.store.add(3f32);

    let const_def_a = Arc::new(TaskDefinition {
        queue_params: vec![],
        data_params: vec![],
        output: TaskOutput {
            type_def: f32::meta_def(),
        },
        callback: |args, queues, output| unsafe {
            std::ptr::write(output.as_mut_ptr() as *mut f32, 2f32);
        },
    });

    let const_def_b = Arc::new(TaskDefinition {
        queue_params: vec![],
        data_params: vec![],
        output: TaskOutput {
            type_def: f32::meta_def(),
        },
        callback: |args, queues, output| unsafe {
            std::ptr::write(output.as_mut_ptr() as *mut f32, 5f32);
        },
    });

    let const_def_c = Arc::new(TaskDefinition {
        queue_params: vec![],
        data_params: vec![],
        output: TaskOutput {
            type_def: f32::meta_def(),
        },
        callback: |args, queues, output| unsafe {
            std::ptr::write(output.as_mut_ptr() as *mut f32, 3f32);
        },
    });

    let const_node_a = runtime
        .main_graph
        .add_task(const_a, const_def_a, vec![], vec![]);
    let const_node_b = runtime
        .main_graph
        .add_task(const_b, const_def_b, vec![], vec![]);
    let const_node_c = runtime
        .main_graph
        .add_task(const_c, const_def_c, vec![], vec![]);

    println!("const_node_a regstered as {:?}", const_node_a);
    println!("const_node_b regstered as {:?}", const_node_b);
    println!("const_node_c regstered as {:?}", const_node_c);

    let add_def = Arc::new(TaskDefinition {
        queue_params: vec![],
        data_params: vec![
            TaskDataParam {
                type_def: f32::meta_def(),
            },
            TaskDataParam {
                type_def: f32::meta_def(),
            },
        ],
        output: TaskOutput {
            type_def: f32::meta_def(),
        },
        callback: |args, queues, output| unsafe {
            let a = f32::from_slice(args[0]);
            let b = f32::from_slice(args[1]);

            let res = a + b;

            std::ptr::write(output.as_mut_ptr() as *mut f32, res);
        },
    });

    let div_def = Arc::new(TaskDefinition {
        queue_params: vec![],
        data_params: vec![
            TaskDataParam {
                type_def: f32::meta_def(),
            },
            TaskDataParam {
                type_def: f32::meta_def(),
            },
        ],
        output: TaskOutput {
            type_def: f32::meta_def(),
        },
        callback: |args, queues, output| unsafe {
            let a = f32::from_slice(args[0]);
            let b = f32::from_slice(args[1]);

            let res = a / b;

            std::ptr::write(output.as_mut_ptr() as *mut f32, res);
        },
    });

    let add_node = runtime.main_graph.add_task(
        const_zero,
        add_def,
        vec![
            TaskDataArg {
                result_of: const_node_a,
                last_frame: false,
                byte_offset: 0,
                byte_size: std::mem::size_of::<f32>(),
            },
            TaskDataArg {
                result_of: const_node_b,
                last_frame: false,
                byte_offset: 0,
                byte_size: std::mem::size_of::<f32>(),
            },
        ],
        vec![],
    );
    let div_node = runtime.main_graph.add_task(
        const_zero,
        div_def,
        vec![
            TaskDataArg {
                result_of: add_node,
                last_frame: false,
                byte_offset: 0,
                byte_size: std::mem::size_of::<f32>(),
            },
            TaskDataArg {
                result_of: const_node_c,
                last_frame: false,
                byte_offset: 0,
                byte_size: std::mem::size_of::<f32>(),
            },
        ],
        vec![],
    );

    println!("add_node regstered as {:?}", add_node);
    println!("div_node regstered as {:?}", div_node);

    runtime.main_graph.sort().expect("Unable to sort graph");
    println!("Task graph sorted");
    for (i, stage) in runtime.main_graph.get_stages().iter().enumerate() {
        println!("stage {}: {:?}", i, stage.tasks);
    }

    runtime.main_graph.run();
    let result = runtime.main_graph.result_of(div_node);
    let result = runtime.main_graph.store.get(result);
    println!("div_result was: {}", unsafe { f32::from_slice(result) });
}
