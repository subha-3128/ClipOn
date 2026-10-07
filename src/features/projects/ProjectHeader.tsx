import {
  ArrowLeft,
  Edit3,
  Sparkles,
  Mic,
  FolderOpen,
  Sliders,
  RefreshCw,
  Check,
} from "lucide-react";
import { AppSection, ProjectDetail, NormalizedTranscript } from "../../types";

interface ProjectHeaderProps {
  detail: ProjectDetail;
  transcript: NormalizedTranscript | null;
  cutCount: number;
  appSection: AppSection;
  onSectionChange: (sec: AppSection) => void;
  onBack: () => void;
  onRename: (id: string) => void;
  onOpenClipsFolder: () => void;
  onOpenSettings: () => void;
  onRefresh: (id: string) => void;
  fileName: (path: string) => string;
  formatTime: (sec: number) => string;
}

export function ProjectHeader({
  detail,
  transcript,
  cutCount,
  appSection,
  onSectionChange,
  onBack,
  onRename,
  onOpenClipsFolder,
  onOpenSettings,
  onRefresh,
  fileName,
  formatTime,
}: ProjectHeaderProps) {
  return (
    <>
      <header className="workspace-topbar">
        <div className="topbar-left">
          <button
            className="back-btn"
            onClick={onBack}
            title="Back to All Projects"
          >
            <ArrowLeft size={16} />
            <span>Projects</span>
          </button>
          <span className="breadcrumb-slash">/</span>
          <div className="topbar-title-group">
            <h2
              className="project-editable-title"
              onClick={() => onRename(detail.project.id)}
              title="Click to rename project"
            >
              {detail.project.name || fileName(detail.project.sourcePath)}
              <Edit3 size={13} className="title-edit-icon" />
            </h2>
            <div className="project-meta-pills">
              <span className="meta-pill">
                {detail.project.sourceDuration
                  ? formatTime(detail.project.sourceDuration)
                  : "Probing..."}
              </span>
              <span className="meta-pill">
                {detail.project.captionStyle || "Modern Box"}
              </span>
              <span className="meta-pill status-pill">
                {detail.project.status}
              </span>
            </div>
          </div>
        </div>

        <div className="topbar-right">
          <div className="topbar-mode-toggle">
            <button
              className={`topbar-mode-tab ${appSection === "shorts" ? "active" : ""}`}
              onClick={() => onSectionChange("shorts")}
              title="Switch to Shorts & Reels Studio"
            >
              <Sparkles size={12} />
              <span>Shorts & Reels</span>
            </button>
            <button
              className={`topbar-mode-tab podcast ${appSection === "podcast" ? "active" : ""}`}
              onClick={() => onSectionChange("podcast")}
              title="Switch to Dynamic Podcast Studio (1–3 Person Reframe)"
            >
              <Mic size={12} />
              <span>Dynamic Podcast</span>
            </button>
          </div>

          <button
            className="topbar-action-btn"
            onClick={onOpenClipsFolder}
            title="Open Project Clips Folder in Finder"
          >
            <FolderOpen size={15} />
            <span>Clips Folder</span>
          </button>
          <button
            className="topbar-action-btn"
            onClick={onOpenSettings}
            title="Studio Settings"
          >
            <Sliders size={15} />
            <span>Config</span>
          </button>
          <button
            className="topbar-icon-btn"
            onClick={() => onRefresh(detail.project.id)}
            title="Refresh Studio State"
          >
            <RefreshCw size={15} />
          </button>
        </div>
      </header>

      {/* 4-Stage Studio Pipeline Tracker */}
      <div className="pipeline-tracker">
        <div
          className={`pipeline-step ${detail.project.sourcePath ? "complete" : ""}`}
        >
          <div className="step-circle">
            {detail.project.sourcePath ? <Check size={12} /> : "1"}
          </div>
          <div className="step-content">
            <span className="step-title">Source Loaded</span>
            <span className="step-sub">
              {fileName(detail.project.sourcePath)}
            </span>
          </div>
        </div>
        <div className="pipeline-connector" />

        <div className={`pipeline-step ${detail.transcript ? "complete" : ""}`}>
          <div className="step-circle">
            {detail.transcript ? <Check size={12} /> : "2"}
          </div>
          <div className="step-content">
            <span className="step-title">Transcription</span>
            <span className="step-sub">
              {transcript
                ? `${transcript.segments.length} segments`
                : "Pending"}
            </span>
          </div>
        </div>
        <div className="pipeline-connector" />

        <div
          className={`pipeline-step ${detail.candidates.length > 0 ? "complete" : ""}`}
        >
          <div className="step-circle">
            {detail.candidates.length > 0 ? <Check size={12} /> : "3"}
          </div>
          <div className="step-content">
            <span className="step-title">Viral Moments</span>
            <span className="step-sub">
              {detail.candidates.length
                ? `${detail.candidates.length} found`
                : "Pending"}
            </span>
          </div>
        </div>
        <div className="pipeline-connector" />

        <div className={`pipeline-step ${cutCount > 0 ? "complete" : ""}`}>
          <div className="step-circle">
            {cutCount > 0 ? <Check size={12} /> : "4"}
          </div>
          <div className="step-content">
            <span className="step-title">Rendered Clips</span>
            <span className="step-sub">
              {cutCount > 0 ? `${cutCount} ready` : "Not cut"}
            </span>
          </div>
        </div>
      </div>
    </>
  );
}
