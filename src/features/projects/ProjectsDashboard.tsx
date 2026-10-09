import { useState, useMemo } from "react";
import {
  FileVideo,
  Youtube,
  Search,
  X,
  Clapperboard,
  Loader2,
  UploadCloud,
  Sparkles,
  Zap,
  Trash2,
  Edit3,
  Play,
  Film,
} from "lucide-react";
import { BusyState, EnvironmentStatus, Project } from "../../types";

interface ProjectsDashboardProps {
  projects: Project[];
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
          <h2>Video Projects</h2>
          <p>
            Transform long videos, podcasts, and streams into viral short clips
            for TikTok, Reels, and Shorts.
          </p>
        </div>
        <div className="home-header-actions">
          <button
            className="btn-minimal-primary"
            onClick={onImportMedia}
            disabled={busy !== "idle"}
          >
            {busy === "import" ? (
              <Loader2 className="spin" size={15} />
            ) : (
              <UploadCloud size={15} />
            )}
            Import Recording
          </button>
          <button
            className="btn-minimal-secondary"
            onClick={onOpenYoutubeModal}
            disabled={busy !== "idle" || !environment?.hasYtdlp}
            title={
              !environment?.hasYtdlp
                ? "yt-dlp required for YouTube import"
                : "Download video directly from YouTube"
            }
          >
            <Youtube size={15} />
            Import from YouTube
          </button>
        </div>
      </header>

      {/* Search & Metrics bar */}
      <div className="dashboard-metrics-bar">
        <div className="dashboard-search-container">
          <Search size={14} className="dashboard-search-icon" />
          <input
            type="text"
            placeholder="Search projects by name or file..."
            value={projectSearch}
            onChange={(e) => setProjectSearch(e.target.value)}
          />
          {projectSearch && (
            <button
              onClick={() => setProjectSearch("")}
              className="clear-search"
              aria-label="Clear search"
            >
              <X size={13} />
            </button>
          )}
        </div>

        <div className="dashboard-stats-pills">
          <span className="stats-pill">
            <Film size={12} />
            {projects.length} {projects.length === 1 ? "Project" : "Projects"}
          </span>
          <span className="stats-pill">
            <Zap size={12} />
            Engine:{" "}
            {transcriptionEngine === "local" ? "Whisper Offline" : "Deepgram"}
          </span>
        </div>
      </div>

      {filteredProjects.length > 0 ? (
        <div className="projects-grid">
          {filteredProjects.map((project) => {
            const name = project.name || fileName(project.sourcePath);
            return (
              <article key={project.id} className="project-card">
                <div
                  className="project-card-banner"
                  onClick={() => void onSelectProject(project.id)}
                  role="button"
                  tabIndex={0}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      void onSelectProject(project.id);
                    }
                  }}
                  title={`Open studio for ${name}`}
                >
                  <div className="project-banner-icon">
                    <FileVideo size={28} />
                  </div>
                  <div className="project-banner-play-hint">
                    <Play size={16} fill="currentColor" />
                    <span>Open Studio</span>
                  </div>
                  <span
                    className={`project-card-status status-${project.status.toLowerCase()}`}
                  >
                    {project.status}
                  </span>
                </div>

                <div className="project-card-content">
                  <h3
                    className="project-card-title truncate"
                    title={name}
                    onClick={() => void onSelectProject(project.id)}
                  >
                    {name}
                  </h3>
                  <div className="project-card-meta">
                    <span className="project-timecode">
                      {project.sourceDuration
                        ? formatTime(project.sourceDuration)
                        : "Probing..."}
                    </span>
                    <span className="project-date">
                      {formatDate(project.createdAt)}
                    </span>
                  </div>

                  <div className="project-card-actions">
                    <button
                      className="action-btn open-btn"
                      onClick={() => void onSelectProject(project.id)}
                    >
                      <Play size={12} fill="currentColor" />
                      <span>Open Studio</span>
                    </button>
                    <button
                      className="action-btn rename-btn"
                      onClick={() => void onRenameProject(project.id)}
                      title="Rename project"
                      aria-label="Rename project"
                    >
                      <Edit3 size={13} />
                      <span>Rename</span>
                    </button>
                    <button
                      className="action-btn delete-btn"
                      onClick={() => void onDeleteProject(project.id)}
                      title="Delete project"
                      aria-label="Delete project"
                    >
                      <Trash2 size={13} />
                      <span>Delete</span>
                    </button>
                  </div>
                </div>
              </article>
            );
          })}
        </div>
      ) : projects.length > 0 ? (
        <div className="empty-dashboard-state">
          <Search size={36} className="empty-state-icon" />
          <h3>No projects match "{projectSearch}"</h3>
          <p>Check for typos or clear your search to see all projects.</p>
          <button
            className="btn-minimal-secondary"
            onClick={() => setProjectSearch("")}
          >
            Clear Search
          </button>
        </div>
      ) : (
        /* Rich Creator Hero Dropzone */
        <div className="dashboard-hero-dropzone">
          <div className="hero-dropzone-inner">
            <div className="hero-icon-bubble">
              <UploadCloud size={40} />
            </div>
            <h3>Create Your First Viral Short</h3>
            <p className="hero-subtitle">
              Import a local podcast, stream, or video file, or paste a YouTube
              URL. ClipOn automatically identifies viral hooks and renders 9:16
              vertical clips with animated subtitles.
            </p>

            <div className="hero-actions">
              <button
                className="hero-primary-btn"
                onClick={onImportMedia}
                disabled={busy !== "idle"}
              >
                {busy === "import" ? (
                  <Loader2 className="spin" size={16} />
                ) : (
                  <FileVideo size={16} />
                )}
                <span>Select Video File</span>
              </button>
              <button
                className="hero-secondary-btn"
                onClick={onOpenYoutubeModal}
                disabled={busy !== "idle" || !environment?.hasYtdlp}
                title={
                  !environment?.hasYtdlp
                    ? "yt-dlp required"
                    : "Download video directly from YouTube"
                }
              >
                <Youtube size={16} />
                <span>Import from YouTube</span>
              </button>
            </div>

            <div className="hero-formats">
              <span className="formats-label">Supported Formats:</span>
              <span className="format-tag">.MP4</span>
              <span className="format-tag">.MOV</span>
              <span className="format-tag">.WEBM</span>
              <span className="format-tag">.MKV</span>
            </div>

            <div className="hero-features-grid">
              <div className="feature-item">
                <Sparkles size={16} className="feature-icon" />
                <div>
                  <strong>AI Hook Detection</strong>
                  <p>Spots punchy soundbites & peak audience retention</p>
                </div>
              </div>
              <div className="feature-item">
                <Zap size={16} className="feature-icon" />
                <div>
                  <strong>Apple Silicon GPU</strong>
                  <p>Hardware accelerated VideoToolbox h264 export</p>
                </div>
              </div>
              <div className="feature-item">
                <Clapperboard size={16} className="feature-icon" />
                <div>
                  <strong>9:16 Smart Reframe</strong>
                  <p>Center or face-tracking vertical crop with punch-zoom</p>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
