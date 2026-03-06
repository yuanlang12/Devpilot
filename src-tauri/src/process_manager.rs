use crate::registry::{ProjectStatus, RegisteredProject};
use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;

fn logs_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = home.join(".devpilot").join("logs");
    fs::create_dir_all(&dir).ok();
    dir
}

fn log_file_path(project_id: &str) -> PathBuf {
    logs_dir().join(format!("{}.log", project_id))
}

/// 检查端口是否可用
fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// 从偏好端口开始，找到一个可用端口
fn find_available_port(preferred: u16) -> u16 {
    for port in preferred..preferred + 100 {
        if is_port_available(port) {
            return port;
        }
    }
    // fallback：让系统分配
    if let Ok(listener) = TcpListener::bind("127.0.0.1:0") {
        if let Ok(addr) = listener.local_addr() {
            return addr.port();
        }
    }
    preferred
}

/// 启动项目
pub fn start_project(project: &mut RegisteredProject) -> Result<(), Box<dyn std::error::Error>> {
    // 找到可用端口
    let port = find_available_port(project.preferred_port);

    let log_path = log_file_path(&project.id);
    let log_file = fs::File::create(&log_path)?;
    let log_err = log_file.try_clone()?;

    // 检测包管理器
    let project_path = std::path::Path::new(&project.path);
    let cmd = adjust_command_for_package_manager(&project.start_command, project_path);

    // 使用 shell 启动命令，注入 PORT 环境变量
    let child = Command::new("sh")
        .args(["-c", &cmd])
        .current_dir(&project.path)
        .env("PORT", port.to_string())
        .stdout(log_file)
        .stderr(log_err)
        .spawn()?;

    project.pid = Some(child.id());
    project.actual_port = Some(port);
    project.status = ProjectStatus::Running;

    Ok(())
}

/// 根据项目目录中的锁文件调整启动命令
fn adjust_command_for_package_manager(cmd: &str, project_path: &std::path::Path) -> String {
    if project_path.join("pnpm-lock.yaml").exists() {
        cmd.replace("npm run", "pnpm run").replace("npm start", "pnpm start")
    } else if project_path.join("yarn.lock").exists() {
        cmd.replace("npm run", "yarn").replace("npm start", "yarn start")
    } else if project_path.join("bun.lockb").exists() || project_path.join("bun.lock").exists() {
        cmd.replace("npm run", "bun run").replace("npm start", "bun start")
    } else {
        cmd.to_string()
    }
}

/// 停止项目
pub fn stop_project(project: &mut RegisteredProject) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(pid) = project.pid {
        // 先尝试 SIGTERM 优雅关闭整个进程组
        let _ = Command::new("kill")
            .args(["--", &format!("-{}", pid)])
            .output();

        // 等待一下再 kill
        std::thread::sleep(std::time::Duration::from_millis(500));

        // 确保彻底关闭
        let _ = Command::new("kill")
            .args(["-9", &format!("-{}", pid)])
            .output();

        // 也尝试通过端口来 kill（以防进程组没生效）
        if let Some(port) = project.actual_port {
            let _ = Command::new("sh")
                .args(["-c", &format!("lsof -ti :{} | xargs kill -9 2>/dev/null", port)])
                .output();
        }
    }

    project.pid = None;
    project.actual_port = None;
    project.status = ProjectStatus::Stopped;

    Ok(())
}

