use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevServer {
    pub pid: u32,
    pub port: u16,
    pub command: String,
    pub cwd: Option<String>,
    pub project_name: Option<String>,
    pub project_type: Option<String>,
    /// 同项目下的其他服务端口（前端/后端分组用）
    pub related_ports: Vec<RelatedPort>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedPort {
    pub port: u16,
    pub role: String, // "frontend" / "backend" / "other"
}

/// 扫描本机所有正在监听的开发服务器端口（3000-9999）
pub fn scan_listening_ports() -> Result<Vec<DevServer>, Box<dyn std::error::Error>> {
    let output = Command::new("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-nP", "-Fpcn"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // 第一步：收集所有原始服务信息
    let mut raw_servers: Vec<RawServer> = Vec::new();
    let mut current_pid: Option<u32> = None;
    let mut current_command: Option<String> = None;

    for line in stdout.lines() {
        if let Some(pid_str) = line.strip_prefix('p') {
            current_pid = pid_str.parse().ok();
            current_command = None;
        } else if let Some(cmd) = line.strip_prefix('c') {
            current_command = Some(cmd.to_string());
        } else if let Some(name) = line.strip_prefix('n') {
            let pid = match current_pid {
                Some(p) => p,
                None => continue,
            };
            let cmd = match &current_command {
                Some(c) => c.clone(),
                None => continue,
            };

            if let Some(port_str) = name.rsplit(':').next() {
                if let Ok(port) = port_str.parse::<u16>() {
                    if (3000..=9999).contains(&port) {
                        let cmd_lower = cmd.to_lowercase();
                        if is_dev_server_process(&cmd_lower) {
                            // 避免同 PID 重复
                            if !raw_servers.iter().any(|s| s.pid == pid) {
                                let cwd = get_process_cwd(pid);
                                let full_cmd = get_full_command(pid);
                                // 如果 cwd 是 / 或空，尝试从命令行参数推断
                                let project_dir = match &cwd {
                                    Some(d) if d != "/" => Some(d.clone()),
                                    _ => infer_project_dir_from_cmd(&full_cmd),
                                };

                                raw_servers.push(RawServer {
                                    pid,
                                    port,
                                    command: cmd,
                                    full_command: full_cmd,
                                    project_dir,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // 第二步：按项目目录分组
    let mut project_groups: HashMap<String, Vec<&RawServer>> = HashMap::new();
    let mut ungrouped: Vec<&RawServer> = Vec::new();

    for server in &raw_servers {
        if let Some(ref dir) = server.project_dir {
            // 统一到项目根目录（去掉 /frontend, /backend 等子目录）
            let root = normalize_project_root(dir);
            project_groups.entry(root).or_default().push(server);
        } else {
            ungrouped.push(server);
        }
    }

    // 第三步：构建输出
    let mut result: Vec<DevServer> = Vec::new();

    for (project_root, servers) in &project_groups {
        if servers.len() == 1 {
            // 单个服务，使用规范化后的项目根目录来获取名称
            let s = servers[0];
            let (project_name, project_type) = detect_project_info(project_root);
            result.push(DevServer {
                pid: s.pid,
                port: s.port,
                command: s.command.clone(),
                cwd: Some(project_root.clone()),
                project_name,
                project_type,
                related_ports: vec![],
            });
        } else {
            // 多个服务属于同一项目，合并
            let primary = pick_primary_server(servers);
            let (project_name, project_type) = detect_project_info(project_root);

            let related: Vec<RelatedPort> = servers
                .iter()
                .filter(|s| s.pid != primary.pid)
                .map(|s| RelatedPort {
                    port: s.port,
                    role: detect_service_role(s),
                })
                .collect();

            result.push(DevServer {
                pid: primary.pid,
                port: primary.port,
                command: primary.command.clone(),
                cwd: Some(project_root.clone()),
                project_name,
                project_type,
                related_ports: related,
            });
        }
    }

    for s in ungrouped {
        result.push(DevServer {
            pid: s.pid,
            port: s.port,
            command: s.command.clone(),
            cwd: s.project_dir.clone(),
            project_name: Some(s.command.clone()),
            project_type: None,
            related_ports: vec![],
        });
    }

    result.sort_by_key(|s| s.port);
    Ok(result)
}

struct RawServer {
    pid: u32,
    port: u16,
    command: String,
    full_command: String,
    project_dir: Option<String>,
}

fn is_dev_server_process(cmd: &str) -> bool {
    let dev_processes = [
        "node", "next", "vite", "python", "ruby", "php", "go", "java", "deno", "bun",
    ];
    dev_processes.iter().any(|p| cmd.contains(p))
}

fn get_process_cwd(pid: u32) -> Option<String> {
    let output = Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix('n') {
            if path != "/" {
                return Some(path.to_string());
            }
        }
    }
    None
}

fn get_full_command(pid: u32) -> String {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-ww", "-o", "args="])
        .output()
        .ok();

    match output {
        Some(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        None => String::new(),
    }
}

/// 从命令行参数推断项目目录
fn infer_project_dir_from_cmd(full_cmd: &str) -> Option<String> {
    // 查找命令中的绝对路径
    for part in full_cmd.split_whitespace() {
        if part.starts_with('/') && part.contains("/node_modules/") {
            // 例如 /Users/x/project/node_modules/.bin/vite -> /Users/x/project
            if let Some(idx) = part.find("/node_modules/") {
                return Some(part[..idx].to_string());
            }
        }
        if part.starts_with('/') && part.contains("/.next/") {
            if let Some(idx) = part.find("/.next/") {
                return Some(part[..idx].to_string());
            }
        }
    }
    None
}

/// 规范化项目根目录：如果路径以 /frontend, /backend, /server, /client 结尾，取父目录
fn normalize_project_root(dir: &str) -> String {
    let path = std::path::Path::new(dir);
    let leaf = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let subdirs = ["frontend", "backend", "server", "client", "web", "app", "api"];
    if subdirs.contains(&leaf.as_str()) {
        if let Some(parent) = path.parent() {
            return parent.to_string_lossy().to_string();
        }
    }
    dir.to_string()
}

/// 选择主服务（前端优先展示）
fn pick_primary_server<'a>(servers: &[&'a RawServer]) -> &'a RawServer {
    // 优先选择前端服务
    for s in servers {
        let role = detect_service_role(s);
        if role == "frontend" {
            return s;
        }
    }
    // 否则选端口最小的
    servers
        .iter()
        .min_by_key(|s| s.port)
        .unwrap_or(&servers[0])
}

/// 判断服务角色：前端/后端
fn detect_service_role(server: &RawServer) -> String {
    let cmd = server.full_command.to_lowercase();
    let dir = server
        .project_dir
        .as_deref()
        .unwrap_or("")
        .to_lowercase();

    // 前端判断
    if cmd.contains("next-server")
        || cmd.contains("vite")
        || cmd.contains("webpack")
        || cmd.contains("react-scripts")
        || dir.ends_with("/frontend")
        || dir.ends_with("/client")
        || dir.ends_with("/web")
    {
        return "frontend".to_string();
    }

    // 后端判断
    if cmd.contains("uvicorn")
        || cmd.contains("gunicorn")
        || cmd.contains("flask")
        || cmd.contains("django")
        || cmd.contains("express")
        || cmd.contains("fastapi")
        || cmd.contains("nest")
        || dir.ends_with("/backend")
        || dir.ends_with("/server")
        || dir.ends_with("/api")
    {
        return "backend".to_string();
    }

    "other".to_string()
}

fn detect_project_info(dir: &str) -> (Option<String>, Option<String>) {
    let path = std::path::Path::new(dir);

    // 尝试读取 package.json（先在当前目录，再在子目录 frontend/）
    let candidates = [
        path.join("package.json"),
        path.join("frontend").join("package.json"),
        path.join("client").join("package.json"),
    ];

    for pkg_path in &candidates {
        if let Ok(content) = std::fs::read_to_string(pkg_path) {
            if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                // 始终使用文件夹名作为显示名称，对用户更直观
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string());

                let project_type =
                    detect_js_project_type(&pkg, pkg_path.parent().unwrap_or(path));
                return (name, project_type);
            }
        }
    }

    // Python 项目
    if path.join("manage.py").exists() || path.join("backend").join("manage.py").exists() {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string());
        return (name, Some("Django".to_string()));
    }
    if path.join("requirements.txt").exists()
        || path.join("pyproject.toml").exists()
        || path.join("backend").join("requirements.txt").exists()
    {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string());
        return (name, Some("Python".to_string()));
    }

    // Go 项目
    if path.join("go.mod").exists() {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string());
        return (name, Some("Go".to_string()));
    }

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string());
    (name, None)
}

fn detect_js_project_type(pkg: &serde_json::Value, dir: &std::path::Path) -> Option<String> {
    let deps = pkg
        .get("dependencies")
        .unwrap_or(&serde_json::Value::Null);
    let dev_deps = pkg
        .get("devDependencies")
        .unwrap_or(&serde_json::Value::Null);

    if dir.join("next.config.js").exists()
        || dir.join("next.config.mjs").exists()
        || dir.join("next.config.ts").exists()
        || deps.get("next").is_some()
    {
        return Some("Next.js".to_string());
    }
    if dir.join("nuxt.config.ts").exists() || deps.get("nuxt").is_some() {
        return Some("Nuxt".to_string());
    }
    if dir.join("vite.config.ts").exists() || dir.join("vite.config.js").exists() {
        if deps.get("vue").is_some() {
            return Some("Vue + Vite".to_string());
        }
        if deps.get("react").is_some() || dev_deps.get("react").is_some() {
            return Some("React + Vite".to_string());
        }
        return Some("Vite".to_string());
    }
    if deps.get("express").is_some() {
        return Some("Express".to_string());
    }
    if dir.join("nest-cli.json").exists() || deps.get("@nestjs/core").is_some() {
        return Some("NestJS".to_string());
    }
    if deps.get("react").is_some() || dev_deps.get("react").is_some() {
        return Some("React".to_string());
    }
    if deps.get("vue").is_some() {
        return Some("Vue".to_string());
    }
    Some("Node.js".to_string())
}
