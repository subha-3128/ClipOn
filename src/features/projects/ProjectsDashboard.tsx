import { useState, useMemo } from "react";
import {
  FileVideo,
  Youtube,
  Search,
  X,
  Clapperboard,
  Loader2,
} from "lucide-react";
import { AppSection, BusyState, EnvironmentStatus, Project } from "../../types";

interface ProjectsDashboardProps {
  projects: Project[];
  appSection: AppSection;
  busy: BusyState;
  environment: EnvironmentStatus | null;
  transcriptionEngine: "deepgram" | "local";
  onImportMedia: () => void;
  onOpenYoutubeModal: () => void;
  onSelectProject: (id: string) => void;
  onRenameProject: (id: string) => void;
  onDeleteProject: (id: string) => void;
  fileName: (path: string) => string;
  formatTime: (sec: number) => string;
  formatDate: (dateStr: string) => string;
}

export function ProjectsDashboard({
  projects,
  appSection,
  busy,
  environment,
  transcriptionEngine,
  onImportMedia,
  onOpenYoutubeModal,
  onSelectProject,
  onRenameProject,
  onDeleteProject,
  fileName,
  formatTime,
  formatDate,
}: ProjectsDashboardProps) {
  const [projectSearch, setProjectSearch] = useState("");

  const filteredProjects = useMemo(() => {
    if (!projectSearch.trim()) return projects;
    const q = projectSearch.toLowerCase();
    return projects.filter(
      (p) =>
        (p.name && p.name.toLowerCase().includes(q)) ||
        p.sourcePath.toLowerCase().includes(q)
    );
  }, [projects, projectSearch]);

  return (
    <div className="home-dashboard">
      <header className="home-header">
        <div className="home-header-info">
          <h2>
            {appSection === "podcast"
              ? "Podcast Studio (Dynamic 9:16)"
              : "All Projects"}
          </h2>
          <p>
            {appSection === "podcast"
              ? "Transform 16:9 podcasts into dynamic multi-person vertical clips (1, 2, or 3 people) for Instagram Reels."
              : "Select a project below or import a new media file to get started."}
          </p>
        </div>
        <div className="home-header-actions">
          <button
            className="btn-minimal-primary"
            onClick={onImportMedia}
            disabled={busy !== "idle"}
          >
            {busy === "import" ? <Loader2 className="spin" size={15} /> : <FileVideo size={15} />}
            {appSection === "podcast" ? "Import Podcast Video" : "Import Recording"}
          </button>
          <button
            className="btn-minimal-secondary"
            onClick={onOpenYoutubeModal}
            disabled={busy !== "idle" || !environment?.hasYtdlp}
            title={!environment?.hasYtdlp ? "yt-dlp required" : "Download a video from YouTube"}
          >
            <Youtube size={15} />
            {appSection === "podcast" ? "Podcast YouTube URL" : "Import from YouTube"}
          </button>
        </div>
      </header>

      {/* Search & Metrics bar */}
      <div className="dashboard-metrics-bar">
        <div className="dashboard-search-container">
          <Search size={14} className="dashboard-search-icon" />
          <input
            type="text"
            placeholder="Search projects..."
            value={projectSearch}
            onChange={(e) => setProjectSearch(e.target.value)}
          />
          {projectSearch && (
            <button onClick={() => setProjectSearch("")} className="clear-search">
              <X size={13} />
            </button>
          )}
        </div>

        <div className="dashboard-stats-pills">
          <span className="stats-pill">{projects.length} Total Projects</span>
          <span className="stats-pill">
            Engine: {transcriptionEngine === "local" ? "Whisper Offline" : "Deepgram"}
          </span>
        </div>
      </div>

      {filteredProjects.length > 0 ? (
        <div className="projects-grid">
          {filteredProjects.map((project) => {
            const name = project.name || fileName(project.sourcePath);
            return (
              <article key={project.id} className="project-card">
                <div className="project-card-header">
                  <FileVideo size={20} className="project-card-icon" />
                  <span className="project-card-status">{project.status}</span>
                </div>
                <h3 className="project-card-title truncate" title={name}>
                  {name}
                </h3>
                <div className="project-card-meta">
                  <span>
                    {project.sourceDuration
                      ? formatTime(project.sourceDuration)
                      : "Probing..."}
                  </span>
                  <span>{formatDate(project.createdAt)}</span>
                </div>
                <div className="project-card-actions">
                  <button
                    className="action-btn open-btn"
                    onClick={() => void onSelectProject(project.id)}
                  >
                    Open Studio
                  </button>
                  <button
                    className="action-btn rename-btn"
                    onClick={() => void onRenameProject(project.id)}
                  >
                    Rename
                  </button>
                  <button
                    className="action-btn delete-btn"
                    onClick={() => void onDeleteProject(project.id)}
                  >
                    Delete
                  </button>
                </div>
              </article>
            );
          })}
        </div>
      ) : projects.length > 0 ? (
        <div className="empty-dashboard-state">
          <Search size={36} className="empty-state-icon" />
          <h3>No projects match "{projectSearch}"</h3>
          <p>Try clearing your search query.</p>
        </div>
      ) : (
        <div className="empty-dashboard-state">
          <Clapperboard size={44} className="empty-state-icon" />
          <h3>No projects found</h3>
          <p>Import a recording or paste a YouTube link to generate viral short clips.</p>
        </div>
      )}
    </div>
  );
}
