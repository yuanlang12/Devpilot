import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ProjectCard } from "./components/ProjectCard";
import { EmptyState } from "./components/EmptyState";
import { open } from "@tauri-apps/plugin-dialog";

interface RelatedPort {
  port: number;
  role: string;
}

interface DevServer {
  pid: number;
  port: number;
  command: string;
  full_command: string | null;
  cwd: string | null;
  project_name: string | null;
  project_type: string | null;
  related_ports: RelatedPort[];
}

interface RegisteredProject {
  id: string;
  name: string;
  path: string;
  start_command: string;
  preferred_port: number;
  project_type: string | null;
  pid: number | null;
  actual_port: number | null;
  status: "running" | "stopped" | "error";
}

function App() {
  const [discovered, setDiscovered] = useState<DevServer[]>([]);
  const [projects, setProjects] = useState<RegisteredProject[]>([]);
  const [scanning, setScanning] = useState(false);

  const scanServers = useCallback(async () => {
    setScanning(true);
    try {
      const servers = await invoke<DevServer[]>("scan_dev_servers");
      setDiscovered(servers);

      const syncData = servers
        .filter((s) => s.cwd)
        .map((s) => ({ path: s.cwd!, pid: s.pid, port: s.port, full_command: s.full_command }));
      if (syncData.length > 0) {
        try {
          await invoke("sync_discovered_projects", { servers: syncData });
        } catch {
          // 同步失败不影响主流程
        }
      }
    } catch (e) {
      console.error("扫描失败:", e);
    }
    setScanning(false);
  }, []);

  const loadProjects = useCallback(async () => {
    try {
      const list = await invoke<RegisteredProject[]>("get_projects");
      setProjects(list);
    } catch (e) {
      console.error("加载项目失败:", e);
    }
  }, []);

  useEffect(() => {
    scanServers();
    loadProjects();

    const interval = setInterval(() => {
      scanServers();
      loadProjects();
    }, 5000);

    return () => clearInterval(interval);
  }, [scanServers, loadProjects]);

  const handleAddProject = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "选择项目文件夹",
      });
      if (selected) {
        await invoke("add_project", { path: selected, name: null });
        await loadProjects();
      }
    } catch (e) {
      console.error("添加项目失败:", e);
    }
  };

  const handleStartProject = async (id: string) => {
    try {
      await invoke("start_project", { id });
      await loadProjects();
    } catch (e) {
      console.error("启动失败:", e);
    }
  };

  const handleStopProject = async (id: string) => {
    try {
      await invoke("stop_project", { id });
      await loadProjects();
    } catch (e) {
      console.error("停止失败:", e);
    }
  };

  const handleRemoveProject = async (id: string) => {
    try {
      await invoke("remove_project", { id });
      await loadProjects();
    } catch (e) {
      console.error("移除失败:", e);
    }
  };

  const handleOpenBrowser = async (port: number) => {
    try {
      await invoke("open_in_browser", { url: `http://localhost:${port}` });
    } catch (e) {
      console.error("打开浏览器失败:", e);
    }
  };

  const handleStopDiscovered = async (pid: number) => {
    try {
      await invoke("kill_process", { pid });
      setTimeout(async () => {
        await scanServers();
        await loadProjects();
      }, 500);
    } catch (e) {
      console.error("停止进程失败:", e);
    }
  };

  const discoveredPaths = new Set(discovered.map((d) => d.cwd).filter(Boolean));

  const recentlyStarted = projects.filter(
    (p) => p.status === "running" && !discoveredPaths.has(p.path)
  );

  const stoppedProjects = projects.filter((p) => p.status !== "running");

  const hasRunning = discovered.length > 0 || recentlyStarted.length > 0;
  const isEmpty = !hasRunning && stoppedProjects.length === 0;

  return (
    <>
      <div className="titlebar">
        <span className="titlebar-title">DevPilot</span>
      </div>

      <div className="app-container">
        <div className="header">
          <div className="header-left">
            <h1>项目管理</h1>
            {hasRunning && (
              <span className="scan-badge">
                <span className="dot" />
                {discovered.length + recentlyStarted.length} 个服务运行中
              </span>
            )}
          </div>
          <div className="header-actions">
            <button className="btn" onClick={() => { scanServers(); loadProjects(); }}>
              {scanning ? <span className="spinner" /> : "↻"} 刷新
            </button>
            <button className="btn" onClick={handleAddProject}>
              + 添加项目
            </button>
          </div>
        </div>

        <div className="content">
          {isEmpty ? (
            <EmptyState
              icon="🔍"
              title="没有发现运行中的开发服务器"
              description="当你用编程工具启动项目后，这里会自动显示"
            />
          ) : (
            <>
              {hasRunning && (
                <>
                  <div className="section-label">运行中</div>
                  {discovered.map((server) => (
                    <ProjectCard
                      key={`d-${server.pid}-${server.port}`}
                      name={server.project_name || server.command}
                      projectType={server.project_type}
                      path={server.cwd}
                      port={server.port}
                      status="running"
                      relatedPorts={server.related_ports}
                      onOpenBrowser={() => handleOpenBrowser(server.port)}
                      onStop={() => handleStopDiscovered(server.pid)}
                    />
                  ))}
                  {recentlyStarted.map((project) => (
                    <ProjectCard
                      key={`r-${project.id}`}
                      name={project.name}
                      projectType={project.project_type}
                      path={project.path}
                      port={project.actual_port}
                      status={project.status}
                      onOpenBrowser={
                        project.actual_port
                          ? () => handleOpenBrowser(project.actual_port!)
                          : undefined
                      }
                      onStop={() => handleStopProject(project.id)}
                    />
                  ))}
                </>
              )}
              {stoppedProjects.length > 0 && (
                <>
                  <div className="section-label">已停止</div>
                  {stoppedProjects.map((project) => (
                    <ProjectCard
                      key={`s-${project.id}`}
                      name={project.name}
                      projectType={project.project_type}
                      path={project.path}
                      port={null}
                      status={project.status}
                      onStart={() => handleStartProject(project.id)}
                      onRemove={() => handleRemoveProject(project.id)}
                    />
                  ))}
                </>
              )}
            </>
          )}
        </div>
      </div>
    </>
  );
}

export default App;
