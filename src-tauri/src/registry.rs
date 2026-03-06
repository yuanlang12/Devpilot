use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredProject {
    pub id: String,
    pub name: String,
    pub path: String,
    pub start_command: String,
    pub preferred_port: u16,
    pub project_type: Option<String>,
    pub pid: Option<u32>,
    pub actual_port: Option<u16>,
    pub status: ProjectStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Running,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRegistry {
    pub projects: Vec<RegisteredProject>,
    pub next_port: u16,
}

impl ProjectRegistry {
    fn registry_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let config_dir = home.join(".devpilot");
        fs::create_dir_all(&config_dir).ok();
        config_dir.join("registry.json")
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::registry_path();
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let reg: ProjectRegistry = serde_json::from_str(&content)?;
            Ok(reg)
        } else {
            Ok(ProjectRegistry {
                projects: vec![],
                next_port: 3000,
            })
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::registry_path();
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn add_project(
        &mut self,
        path: &str,
        name: Option<String>,
    ) -> Result<RegisteredProject, Box<dyn std::error::Error>> {
        // 检查是否已存在
        if self.projects.iter().any(|p| p.path == path) {
            return Err("该项目已添加".into());
        }

        let dir = std::path::Path::new(path);
        if !dir.exists() {
            return Err("目录不存在".into());
        }

        let (detected_name, project_type, start_command) = detect_project(dir);
        let project_name = name.unwrap_or(detected_name);

        let port = self.allocate_port();
        let id = generate_id();

        let project = RegisteredProject {
            id,
            name: project_name,
            path: path.to_string(),
            start_command,
            preferred_port: port,
            project_type,
            pid: None,
            actual_port: None,
            status: ProjectStatus::Stopped,
        };

        self.projects.push(project.clone());
        Ok(project)
    }

    pub fn remove_project(&mut self, id: &str) {
        self.projects.retain(|p| p.id != id);
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut RegisteredProject> {
        self.projects.iter_mut().find(|p| p.id == id)
    }

    /// 同步发现的服务器到注册表：已有则更新运行状态，没有则自动注册
    pub fn sync_discovered(&mut self, path: &str, pid: u32, port: u16) {
        if let Some(existing) = self.projects.iter_mut().find(|p| p.path == path) {
            existing.pid = Some(pid);
            existing.actual_port = Some(port);
            existing.status = ProjectStatus::Running;
        } else if self.add_project(path, None).is_ok() {
            if let Some(project) = self.projects.iter_mut().find(|p| p.path == path) {
                project.pid = Some(pid);
                project.actual_port = Some(port);
                project.status = ProjectStatus::Running;
            }
        }
    }

    /// 刷新所有项目的运行状态：检查进程是否还活着
    pub fn refresh_statuses(&mut self) -> bool {
        let mut changed = false;
        for project in &mut self.projects {
            if project.status == ProjectStatus::Running {
                let alive = project.pid.map_or(false, is_process_alive);
                if !alive {
                    project.pid = None;
                    project.actual_port = None;
                    project.status = ProjectStatus::Stopped;
                    changed = true;
                }
            }
        }
        changed
    }

    fn allocate_port(&mut self) -> u16 {
        let used_ports: Vec<u16> = self.projects.iter().map(|p| p.preferred_port).collect();
        let mut port = self.next_port;
        while used_ports.contains(&port) {
            port += 1;
        }
        self.next_port = port + 1;
        port
    }
}

/// 检查进程是否还活着
fn is_process_alive(pid: u32) -> bool {
    use std::process::Command;
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn detect_project(dir: &std::path::Path) -> (String, Option<String>, String) {
    // 始终使用文件夹名作为项目名
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // 尝试多个位置的 package.json（项目根目录、frontend/、client/ 子目录）
    let candidates = [
        (dir.join("package.json"), ""),
        (dir.join("frontend").join("package.json"), "cd frontend && "),
        (dir.join("client").join("package.json"), "cd client && "),
    ];

    for (pkg_path, cd_prefix) in &candidates {
        if let Ok(content) = fs::read_to_string(pkg_path) {
            if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                let pkg_dir = pkg_path.parent().unwrap_or(dir);
                let deps = pkg.get("dependencies").unwrap_or(&serde_json::Value::Null);
                let dev_deps = pkg.get("devDependencies").unwrap_or(&serde_json::Value::Null);

                // 检测项目类型和启动命令
                if pkg_dir.join("next.config.js").exists()
                    || pkg_dir.join("next.config.mjs").exists()
                    || pkg_dir.join("next.config.ts").exists()
                    || deps.get("next").is_some()
                {
                    return (name, Some("Next.js".to_string()), format!("{}npm run dev", cd_prefix));
                }
                if pkg_dir.join("nuxt.config.ts").exists() || deps.get("nuxt").is_some() {
                    return (name, Some("Nuxt".to_string()), format!("{}npm run dev", cd_prefix));
                }
                if pkg_dir.join("vite.config.ts").exists() || pkg_dir.join("vite.config.js").exists() {
                    let vtype = if deps.get("vue").is_some() {
                        "Vue + Vite"
                    } else if deps.get("react").is_some() || dev_deps.get("react").is_some() {
                        "React + Vite"
                    } else {
                        "Vite"
                    };
                    return (name, Some(vtype.to_string()), format!("{}npm run dev", cd_prefix));
                }
                if pkg_dir.join("nest-cli.json").exists() || deps.get("@nestjs/core").is_some() {
                    return (name, Some("NestJS".to_string()), format!("{}npm run start:dev", cd_prefix));
                }
                if deps.get("express").is_some() {
                    return (name, Some("Express".to_string()), format!("{}npm run dev", cd_prefix));
                }

                if let Some(scripts) = pkg.get("scripts") {
                    if scripts.get("dev").is_some() {
                        return (name, Some("Node.js".to_string()), format!("{}npm run dev", cd_prefix));
                    }
                    if scripts.get("start").is_some() {
                        return (name, Some("Node.js".to_string()), format!("{}npm start", cd_prefix));
                    }
                }

                return (name, Some("Node.js".to_string()), format!("{}npm run dev", cd_prefix));
            }
        }
    }

    // Python 项目（也检查 backend/ 子目录）
    if dir.join("manage.py").exists() {
        return (name, Some("Django".to_string()), "python manage.py runserver".to_string());
    }
    if dir.join("backend").join("manage.py").exists() {
        return (name, Some("Django".to_string()), "cd backend && python manage.py runserver".to_string());
    }
    if dir.join("requirements.txt").exists() || dir.join("pyproject.toml").exists() {
        return (name, Some("Python".to_string()), "python main.py".to_string());
    }
    if dir.join("backend").join("requirements.txt").exists() {
        return (name, Some("Python".to_string()), "cd backend && python main.py".to_string());
    }

    // Go 项目
    if dir.join("go.mod").exists() {
        return (name, Some("Go".to_string()), "go run .".to_string());
    }

    (name, None, "npm run dev".to_string())
}

fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("{:x}", ts)
}
