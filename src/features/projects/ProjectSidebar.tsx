import {
  Clapperboard,
  Sparkles,
  Mic,
  FileVideo,
  Youtube,
  Layers,
  ChevronRight,
  Settings,
  Loader2,
} from "lucide-react";
import { AppSection, BusyState, EnvironmentStatus, Project } from "../../types";

interface ProjectSidebarProps {
  appSection: AppSection;
  onSectionChange: (section: AppSection) => void;
  busy: BusyState;
  environment: EnvironmentStatus | null;
  projects: Project[];
  selectedProjectId?: string | null;
  onSelectProject: (id: string | null) => void;
  onImportMedia: () => void;
  onOpenYoutubeModal: () => void;
  onOpenSettings: () => void;
  fileName: (path: string) => string;
}

export function ProjectSidebar({
  appSection,
  onSectionChange,
  busy,
  environment,
  projects,
  selectedProjectId,
  onSelectProject,
  onImportMedia,
  onOpenYoutubeModal,
  onOpenSettings,
  fileName,
}: ProjectSidebarProps) {
  return (
    <aside className="sidebar">
      {/* Brand mark */}
      <div
        className="brand-row"
        onClick={() => onSelectProject(null)}
        title="Go to All Projects"
      >
        <div className="brand-mark">
          <Clapperboard size={18} />
        </div>
        <div>
          <div className="brand-title">
            <span>ClipOn</span>
            <span className="brand-badge">v0.1.3</span>
          </div>
          <p className="brand-subtitle">Long recording in. Short clips out.</p>
        </div>
      </div>

      {/* Section Switcher: Shorts & Reels vs Podcast */}
      <div className="sidebar-section-header">
        <span>Studio Mode</span>
      </div>
      <div className="sidebar-mode-switcher">
        <button
          className={`sidebar-mode-btn ${appSection === "shorts" ? "active" : ""}`}
          onClick={() => onSectionChange("shorts")}
          title="Shorts & Reels Studio: Single-speaker 9:16 vertical crop"
        >
          <Sparkles size={14} />
          <span>Shorts & Reels</span>
        </button>
        <button
          className={`sidebar-mode-btn podcast ${appSection === "podcast" ? "active" : ""}`}
          onClick={() => onSectionChange("podcast")}
          title="Podcast Studio: 2-Person 9:16 split-screen for table recordings"
        >
          <Mic size={14} />
          <span>Podcast (9:16)</span>
          <span className="mode-pill-split">2-Face</span>
        </button>
      </div>

      {/* Quick Actions */}
      <div className="sidebar-actions">
        <button
          className="sidebar-action-btn primary"
          onClick={onImportMedia}
          disabled={busy !== "idle"}
        >
          {busy === "import" ? (
            <Loader2 className="spin" size={15} />
          ) : (
            <FileVideo size={15} />
          )}
          {appSection === "podcast"
            ? "Import Podcast Video"
            : "Import Recording"}
        </button>
        <button
          className="sidebar-action-btn secondary"
          onClick={onOpenYoutubeModal}
          disabled={busy !== "idle" || !environment?.hasYtdlp}
          title={
            !environment?.hasYtdlp ? "yt-dlp required" : "Import from YouTube"
          }
        >
          <Youtube size={15} />
          {appSection === "podcast" ? "Podcast YouTube URL" : "Import YouTube"}
        </button>
      </div>

      {/* Project List Navigation */}
      <div className="sidebar-section-header">
        <span>Projects ({projects.length})</span>
      </div>

      <section className="project-list" aria-label="Projects">
        <button
          className={`project-nav-item ${!selectedProjectId ? "active" : ""}`}
          onClick={() => onSelectProject(null)}
        >
          <Layers size={14} />
          <span>All Projects</span>
          <ChevronRight size={13} className="nav-arrow" />
        </button>

        {projects.map((project) => (
          <button
            key={project.id}
            className={`project-nav-item ${selectedProjectId === project.id ? "active" : ""}`}
            onClick={() => void onSelectProject(project.id)}
          >
            <FileVideo size={14} />
            <span className="truncate">
              {project.name || fileName(project.sourcePath)}
            </span>
            <ChevronRight size={13} className="nav-arrow" />
          </button>
        ))}
      </section>

      {/* Sidebar Footer with Settings */}
      <div className="sidebar-footer">
        <button
          className="sidebar-action-btn settings-btn"
          onClick={onOpenSettings}
          title="Global Studio Settings"
        >
          <Settings size={15} />
          <span>Settings</span>
        </button>
      </div>
    </aside>
  );
}
