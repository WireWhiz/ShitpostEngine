use clap::Parser;

use notify::{Event, RecommendedWatcher, RecursiveMode, Result, Watcher};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc,
};
use websocket::OwnedMessage;

use crate::server_api::WeaverMessage;

#[path = "../server_api.rs"]
pub mod server_api;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The engine source code location
    engine_dir: String,

    /// Should brane weave contuniue running and provide updates to the binary, but not launch
    /// any child processes
    #[arg(long, default_value_t = false)]
    watch: bool,

    /// Launch the brane editor and provide hot-reload support
    #[arg(long, default_value_t = false)]
    editor: bool,
    /*
    /// Launch the brane client and provide hot-reload support
    #[arg(long, default_value_t = false)]
    client: bool,
    */
}

fn main() {
    let args = Args::parse();

    let update_server = websocket::sync::Server::bind("localhost:2001").unwrap();
    let mut clients = Vec::new();
    let (clients_tx, clients_rx) = mpsc::channel();
    let accept_thread = std::thread::spawn(move || {
        for connection in update_server {
            let Ok(connection) = connection else {
                continue;
            };
            let _ = clients_tx.send(connection.accept().unwrap());
        }
    });

    let (tx, rx) = mpsc::channel::<Result<Event>>();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(tx).unwrap();

    let engine_dir = PathBuf::from(&args.engine_dir).canonicalize().unwrap();

    let modules_dir = engine_dir.join("brane_runtime").join("modules");
    let editor_dir = engine_dir.join("brane_editor");
    let _client_dir = engine_dir.join("brane_editor");
    let _server_dir = engine_dir.join("brane_server");

    Command::new("cargo")
        .args(vec!["build"])
        .current_dir(&engine_dir)
        .spawn()
        .expect("Failed to cargo build")
        .wait_with_output()
        .unwrap();

    let hotreload_dir = engine_dir.join("target").join("hotreload");
    std::fs::create_dir_all(&hotreload_dir).unwrap();
    copy_stdlib(&hotreload_dir);

    let editor_process = if args.editor {
        Some(
            Command::new("cargo")
                .args(vec!["run", "--bin", "brane_editor"])
                .current_dir(engine_dir)
                .spawn()
                .expect("Failed to start child process"),
        )
    } else {
        None
    };

    // Add a path to be watched. All files and directories at that path and below will be monitored for changes.
    watcher
        .watch(Path::new(&modules_dir), RecursiveMode::Recursive)
        .unwrap();

    println!("Watching {}. Press Ctrl+C to exit.", args.engine_dir);
    let mut lib_count = 0;
    for res in rx {
        while let Ok(new_client) = clients_rx.try_recv() {
            clients.push(new_client);
        }

        match res {
            Ok(event) => match &event.kind {
                notify::EventKind::Any => {}
                notify::EventKind::Access(access_kind) => {}
                notify::EventKind::Create(create_kind) => {}
                notify::EventKind::Modify(modify_kind) => {
                    for path in event.paths {
                        let Some(ext) = path.extension() else {
                            continue;
                        };
                        if ext == "rs" {
                            let module_name =
                                path.file_stem().unwrap().to_str().unwrap().to_string();
                            let lib_name = versioned_artifact_name(&module_name, lib_count);
                            let lib_path = hotreload_dir.join(lib_name);
                            lib_count += 1;

                            //Try compile
                            let res = compile(CompileJob {
                                source: path,
                                output: lib_path.clone(),
                                crate_name: module_name.clone(),
                                extern_flags: vec![
                                    ("cargodeps".into(), "target/debug/libcargodeps.rlib".into()),
                                    (
                                        "brane_weaver".into(),
                                        "target/debug/libbrane_weaver.rlib".into(),
                                    ),
                                    (
                                        "proc_brane_weaver".into(),
                                        format!(
                                            "target/debug/proc_brane_weaver.{}",
                                            dylib_extension()
                                        ),
                                    ),
                                ],
                                search_paths: vec![
                                    "dependency=target/debug/deps".into(),
                                    "dependency=target/debug".into(),
                                ],
                                incremental_dir: hotreload_dir
                                    .join("incremental")
                                    .join(&module_name),
                            });
                            println!(
                                "compiled with result {} in {}ms",
                                res.success, res.elapsed_ms
                            );
                            println!("{}", res.diagnostics);
                            if res.success {
                                println!("Emitted {}", lib_path.display());
                                let msg = WeaverMessage::ReloadModule {
                                    module_name,
                                    dynamic_lib_path: lib_path.to_string_lossy().into(),
                                };
                                let msg = OwnedMessage::Text(serde_json::to_string(&msg).unwrap());
                                for client in &mut clients {
                                    client.send_message(&msg);
                                }
                            }
                        }
                    }
                }
                notify::EventKind::Remove(remove_kind) => {}
                notify::EventKind::Other => {}
            },
            Err(e) => println!("watch error: {:?}", e),
        }
    }

    if let Some(ep) = editor_process {
        ep.wait_with_output().unwrap();
    }

    accept_thread.join().unwrap();
}

fn dylib_extension() -> &'static str {
    #[cfg(target_os = "windows")]
    return "dll";
    #[cfg(target_os = "linux")]
    return "so";
    #[cfg(target_os = "macos")]
    return "dylib";
}

fn versioned_artifact_name(name: &str, version: u64) -> PathBuf {
    format!("{}_{}.{}", name, version, dylib_extension()).into()
}

#[derive(Deserialize)]
struct CompileJob {
    source: PathBuf,
    output: PathBuf,
    crate_name: String,
    extern_flags: Vec<(String, String)>, // (name, path) pairs
    search_paths: Vec<String>,
    incremental_dir: PathBuf,
}

#[derive(Serialize, Debug)]
struct CompileResult {
    success: bool,
    elapsed_ms: u64,
    diagnostics: String,
}

fn compile(job: CompileJob) -> CompileResult {
    std::fs::create_dir_all(&job.incremental_dir).unwrap();
    let target_dir = std::path::Path::new(&job.output).parent().unwrap();
    std::fs::create_dir_all(target_dir).unwrap();
    let start = std::time::Instant::now();

    let mut cmd = std::process::Command::new("rustc");

    cmd.args(["--edition", "2021"])
        .args(["--crate-type", "cdylib"])
        .args(["--crate-name", &job.crate_name])
        .args(["-C", "opt-level=0"])
        .args(["-C", "prefer-dynamic"])
        //.args(["-C", "debuginfo=0"])
        .args([
            "-C",
            &format!("incremental={}", job.incremental_dir.display()),
        ]);

    // Cranelift where supported
    if let Some(backend) = codegen_backend() {
        cmd.args(["-Z", &format!("codegen-backend={}", backend)]);
    }

    for (name, path) in &job.extern_flags {
        cmd.arg("--extern").arg(format!("{}={}", name, path));
    }

    for path in &job.search_paths {
        cmd.arg("-L").arg(path);
    }

    cmd.args([
        OsStr::new("-o"),
        job.output.as_os_str(),
        job.source.as_os_str(),
    ]);

    let output = cmd.output().unwrap();

    CompileResult {
        success: output.status.success(),
        elapsed_ms: start.elapsed().as_millis() as u64,
        diagnostics: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[cfg(target_os = "linux")]
fn codegen_backend() -> Option<&'static str> {
    Some("cranelift")
}

#[cfg(not(target_os = "linux"))]
fn codegen_backend() -> Option<&'static str> {
    None
}

fn copy_stdlib(target_dir: &Path) {
    // Get the toolchain's bin dir from rustc's sysroot
    let output = std::process::Command::new("rustc")
        .args(["--print", "target-libdir"])
        .output()
        .unwrap();

    let libdir = PathBuf::from(std::str::from_utf8(&output.stdout).unwrap().trim());

    std::fs::create_dir_all(&target_dir).unwrap();

    // Copy all std-*.dll files (the hash in the name changes per toolchain update)
    for entry in std::fs::read_dir(&libdir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("std-") && (name.ends_with(".dll") || name.ends_with(".pdb")) {
            let dest = target_dir.join(&*name);
            if !dest.exists() {
                std::fs::copy(entry.path(), &dest).unwrap();
            }
        }
    }
}
