use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum WeaverMessage {
    ReloadModule {
        module_name: String,
        dynamic_lib_path: String,
    },
}
