mod scanner;
mod registry;
mod process_manager;

use serde::Deserialize;
use scanner::DevServer;
use registry::{ProjectRegistry, RegisteredProject};

#[tauri::command]
fn scan_dev_servers() -> Result<Vec<DevServer>, String> {
    scanner::scan_listening_ports().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_projects() -> Result<Vec<RegisteredProject>, String> {
    let mut reg = ProjectRegistry::load().map_err(|e| e.to_string())?;
    if reg.refresh_statuses() {
        reg.save().map_err(|e| e.to_string())?;
    }
    Ok(reg.projects)
}

#[derive(Deserialize)]
struct DiscoveredServerSync {
    path: String,
    pid: u32,
    port: u16,
    full_command: Option<String>,
}

#[tauri::command]
fn sync_discovered_projects(servers: Vec<DiscoveredServerSync>) -> Result<(), String> {
    let mut reg = ProjectRegistry::load().map_err(|e| e.to_string())?;
    for server in &servers {
        reg.sync_discovered(&server.path, server.pid, server.port, server.full_command.clone());
    }
    reg.save().map_err(|e| e.to_string())
}

#[tauri::command]
fn add_project(path: String, name: Option<String>) -> Result<RegisteredProject, String> {
    let mut reg = ProjectRegistry::load().map_err(|e| e.to_string())?;
    let project = reg.add_project(&path, name).map_err(|e| e.to_string())?;
    reg.save().map_err(|e| e.to_string())?;
    Ok(project)
}

#[tauri::command]
fn remove_project(id: String) -> Result<(), String> {
    let mut reg = ProjectRegistry::load().map_err(|e| e.to_string())?;
    reg.remove_project(&id);
    reg.save().map_err(|e| e.to_string())
}

#[tauri::command]
fn start_project(id: String) -> Result<RegisteredProject, String> {
    let mut reg = ProjectRegistry::load().map_err(|e| e.to_string())?;
    let project = reg.find_mut(&id).ok_or("项目不存在")?;
    process_manager::start_project(project).map_err(|e| e.to_string())?;
    let result = project.clone();
    reg.save().map_err(|e| e.to_string())?;
    Ok(result)
}

#[tauri::command]
fn stop_project(id: String) -> Result<(), String> {
    let mut reg = ProjectRegistry::load().map_err(|e| e.to_string())?;
    let project = reg.find_mut(&id).ok_or("项目不存在")?;
    process_manager::stop_project(project).map_err(|e| e.to_string())?;
    reg.save().map_err(|e| e.to_string())
}

#[tauri::command]
fn open_in_browser(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| e.to_string())
}

#[tauri::command]
fn kill_process(pid: u32) -> Result<(), String> {
    use std::process::Command;
    let _ = Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output();
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_dev_servers,
            get_projects,
            add_project,
            remove_project,
            start_project,
            stop_project,
            open_in_browser,
            kill_process,
            sync_discovered_projects,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
