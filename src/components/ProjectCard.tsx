interface RelatedPort {
  port: number;
  role: string;
}

interface ProjectCardProps {
  name: string;
  projectType: string | null;
  path: string | null;
  port: number | null;
  status: "running" | "stopped" | "error";
  relatedPorts?: RelatedPort[];
  onStart?: () => void;
  onStop?: () => void;
  onOpenBrowser?: () => void;
  onRemove?: () => void;
}

const roleLabel: Record<string, string> = {
  frontend: "前端",
  backend: "后端",
  other: "服务",
};

export function ProjectCard({
  name,
  projectType,
  path,
  port,
  status,
  relatedPorts = [],
  onStart,
  onStop,
  onOpenBrowser,
  onRemove,
}: ProjectCardProps) {
  const shortPath = path
    ? path.replace(/^\/Users\/[^/]+/, "~")
    : null;

  const hasRelated = relatedPorts.length > 0;

  return (
    <div className="project-card">
      <div className="project-card-header">
        <div className="project-info">
          <span className={`status-dot ${status}`} />
          <span className="project-name">{name}</span>
          {projectType && (
            <span className="project-type-badge">{projectType}</span>
          )}
        </div>
        <div className="project-card-actions">
          {status === "running" && onOpenBrowser && (
            <button className="btn btn-sm" onClick={onOpenBrowser}>
              打开浏览器
            </button>
          )}
          {status === "running" && onStop && (
            <button className="btn btn-sm btn-danger" onClick={onStop}>
              停止
            </button>
          )}
          {status !== "running" && onStart && (
            <button className="btn btn-sm btn-primary" onClick={onStart}>
              启动
            </button>
          )}
          {onRemove && (
            <button className="btn-icon" onClick={onRemove} title="移除项目">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
              </svg>
            </button>
          )}
        </div>
      </div>
      <div className="project-card-meta">
        {port && !hasRelated && (
          <span>
            端口 <span className="port">:{port}</span>
          </span>
        )}
        {port && hasRelated && (
          <span className="port-group">
            <span className="port-item">
              <span className="port-role">前端</span>
              <span className="port">:{port}</span>
            </span>
            {relatedPorts.map((rp) => (
              <span className="port-item" key={rp.port}>
                <span className="port-role">{roleLabel[rp.role] || rp.role}</span>
                <span className="port">:{rp.port}</span>
              </span>
            ))}
          </span>
        )}
        {shortPath && <span className="path">{shortPath}</span>}
      </div>
    </div>
  );
}
