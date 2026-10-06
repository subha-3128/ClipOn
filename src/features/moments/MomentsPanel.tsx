import {
  Mic,
  Sparkles,
  Scissors,
  Zap,
  AudioLines,
  Filter,
  Loader2,
} from "lucide-react";
import {
  AppSection,
  BusyState,
  Candidate,
  Clip,
  InstagramPost,
  ProjectDetail,
  ReframeMode,
  ExportPresetPlatform,
} from "../../types";
import { ExportPresetSelector } from "../export/ExportPresetSelector";
import { JobProgressBar } from "../jobs/JobProgressBar";
import { MomentCard } from "./MomentCard";

interface MomentsPanelProps {
  detail: ProjectDetail;
  appSection: AppSection;
  reframeMode: ReframeMode;
  setReframeMode: (m: ReframeMode) => void;
  busy: BusyState;
  canUseActiveLlm: boolean;
  onFindMoments: () => void;
  onCutSelected: () => void;
  selectedCount: number;
  cutCount: number;
  momentTab: "all" | "selected" | "ready";
  setMomentTab: (t: "all" | "selected" | "ready") => void;
  selectBatch: (type: "top3" | "top5" | "all" | "none") => void;
  punchZoom: boolean;
  setPunchZoom: (z: boolean) => void;
  removeSilence: boolean;
  setRemoveSilence: (s: boolean) => void;
  studioAudio: boolean;
  setStudioAudio: (a: boolean) => void;
  exportPreset: ExportPresetPlatform;
  setExportPreset: (p: ExportPresetPlatform) => void;
  filteredCandidates: Candidate[];
  clipByCandidate: Map<string, Clip>;
  instagramPostByCandidate: Map<string, InstagramPost>;
  renderingCandidateId: string | null;
  publishingCandidateId: string | null;
  hasFfmpeg: boolean;
  formatTime: (sec: number) => string;
  toggleCandidate: (id: string) => void;
  handleOpenSocialKit: (c: Candidate) => void;
  cutCandidate: (id: string) => void;
  openFolder: (path: string) => void;
  handlePublishToInstagram: (id: string) => void;
  onJobComplete: () => void;
  onJobCancel: () => void;
}

export function MomentsPanel({
  detail,
  appSection,
  reframeMode,
  setReframeMode,
  busy,
  canUseActiveLlm,
  onFindMoments,
  onCutSelected,
  selectedCount,
  cutCount,
  momentTab,
  setMomentTab,
  selectBatch,
  punchZoom,
  setPunchZoom,
  removeSilence,
  setRemoveSilence,
  studioAudio,
  setStudioAudio,
  exportPreset,
  setExportPreset,
  filteredCandidates,
  clipByCandidate,
  instagramPostByCandidate,
  renderingCandidateId,
  publishingCandidateId,
  hasFfmpeg,
  formatTime,
  toggleCandidate,
  handleOpenSocialKit,
  cutCandidate,
  openFolder,
  handlePublishToInstagram,
  onJobComplete,
  onJobCancel,
}: MomentsPanelProps) {
  return (
    <section className="studio-panel moments-studio">
      {appSection === "podcast" && (
        <div className="podcast-studio-banner">
          <div className="podcast-banner-header">
            <div className="podcast-badge-icon">
              <Mic size={14} />
            </div>
            <div>
              <div className="podcast-banner-title">Dynamic Podcast Reframing (16:9 → 9:16)</div>
              <p className="podcast-banner-desc">
                Adaptive multi-person timeline reframing: 1 Person (Full 9:16), 2 People (Top/Bottom Split), 3
                People (2 Top + 1 Bottom). Dynamically tracks persistent identities, preserves framing during
                temporary absences, and keeps captions synchronized along dividing seams.
              </p>
            </div>
          </div>
          <div className="podcast-tracking-status-grid">
            <div className="tracking-status-item">
              <span className="dot dot-blue" />
              <span className="tracking-label">Layouts:</span>
              <span className="tracking-val">1P Full / 2P Split / 3P Dynamic</span>
            </div>
            <div className="tracking-status-item">
              <span className="dot dot-purple" />
              <span className="tracking-label">Identities:</span>
              <span className="tracking-val">Persistent Face Tracking</span>
            </div>
            <div className="tracking-status-item">
              <span className="dot dot-green" />
              <span className="tracking-label">Synchronization:</span>
              <span className="tracking-val">1:1 Source Timestamps</span>
            </div>
            <div className="tracking-status-item">
              <span className="dot dot-amber" />
              <span className="tracking-label">Captions:</span>
              <span className="tracking-val">Seam Line Centered</span>
            </div>
          </div>
        </div>
      )}

      <div className="panel-header">
        <div>
          <h3>Viral Moment Candidates</h3>
          <p>
            {detail.candidates.length
              ? `${selectedCount} selected of ${detail.candidates.length}`
              : "Run AI detection to find viral hooks"}
          </p>
        </div>

        <div className="panel-header-actions">
          <button
            className="studio-btn secondary"
            onClick={onFindMoments}
            disabled={busy !== "idle" || !detail.transcript || !canUseActiveLlm}
          >
            {busy === "moments" ? <Loader2 className="spin" size={14} /> : <Sparkles size={14} />}
            Find Moments
          </button>

          <button
            className="studio-btn primary"
            onClick={onCutSelected}
            disabled={busy !== "idle" || selectedCount === 0 || !hasFfmpeg}
          >
            {busy === "cut" ? <Loader2 className="spin" size={14} /> : <Scissors size={14} />}
            Cut Selected ({selectedCount})
          </button>
        </div>
      </div>

      {/* Batch Toolbar & Controls */}
      <div className="moments-controls-bar">
        {/* Top Row: Filter Tabs & Quick Batch Select */}
        <div className="controls-row top-row">
          <div className="tab-pills">
            <button
              className={`tab-pill ${momentTab === "all" ? "active" : ""}`}
              onClick={() => setMomentTab("all")}
            >
              All ({detail.candidates.length})
            </button>
            <button
              className={`tab-pill ${momentTab === "selected" ? "active" : ""}`}
              onClick={() => setMomentTab("selected")}
            >
              Selected ({selectedCount})
            </button>
            <button
              className={`tab-pill ${momentTab === "ready" ? "active" : ""}`}
              onClick={() => setMomentTab("ready")}
            >
              Rendered ({cutCount})
            </button>
          </div>

          <div className="batch-actions">
            <span className="batch-label">Quick Select:</span>
            <button className="batch-btn" onClick={() => selectBatch("top3")}>
              Top 3
            </button>
            <button className="batch-btn" onClick={() => selectBatch("top5")}>
              Top 5
            </button>
            <button className="batch-btn" onClick={() => selectBatch("all")}>
              All
            </button>
            <button className="batch-btn" onClick={() => selectBatch("none")}>
              Clear
            </button>
          </div>
        </div>

        {/* Video Framing Selector */}
        <div className="controls-row bottom-row">
          <div className="reframe-picker">
            <span className="reframe-label">Framing:</span>
            <select
              value={reframeMode}
              onChange={(e) => setReframeMode(e.target.value as ReframeMode)}
              title="Video Framing Aspect Ratio"
            >
              <option value="vertical_crop">Center Crop (9:16)</option>
              <option value="podcast_split">Podcast Studio (Dynamic 1P / 2P / 3P Split 9:16)</option>
              <option value="original">Original Aspect Ratio</option>
            </select>
          </div>
        </div>

        {/* Feature Toggles */}
        <div className="feature-buttons-row">
          <button
            type="button"
            className={`feature-toggle-btn punch-zoom ${punchZoom ? "active" : ""}`}
            onClick={() => {
              const next = !punchZoom;
              setPunchZoom(next);
              localStorage.setItem("clipon_punch_zoom", String(next));
            }}
            title="Auto-Punch 1.14x Retention Zoom Cuts Every 5.5s to Reset Visual Focus"
          >
            <Zap size={12} /> {punchZoom ? "Punch Zoom: ON" : "Punch Zoom: OFF"}
          </button>

          <button
            type="button"
            className={`feature-toggle-btn dead-air ${removeSilence ? "active" : ""}`}
            onClick={() => {
              const next = !removeSilence;
              setRemoveSilence(next);
              localStorage.setItem("clipon_remove_silence", String(next));
            }}
            title="Auto-Detect & Jump-Cut Dead Air / Pauses >0.45s for 20% Faster Clip Retention"
          >
            <Scissors size={12} /> {removeSilence ? "Dead Air Cut: ON" : "Dead Air Cut: OFF"}
          </button>

          <button
            type="button"
            className={`feature-toggle-btn studio-audio ${studioAudio ? "active" : ""}`}
            onClick={() => {
              const next = !studioAudio;
              setStudioAudio(next);
              localStorage.setItem("clipon_studio_audio", String(next));
            }}
            title="Studio Sound Auto-Mastering: -14 LUFS Broadcast Standard & AI Noise Suppression"
          >
            <AudioLines size={12} /> {studioAudio ? "Studio Audio: ON" : "Studio Audio: OFF"}
          </button>
        </div>

        <ExportPresetSelector selectedPreset={exportPreset} onSelectPreset={setExportPreset} />

        <JobProgressBar onJobComplete={onJobComplete} onJobCancel={onJobCancel} />
      </div>

      {/* Candidates Cards List */}
      <div className="candidates-scroll-area">
        {filteredCandidates.length > 0 ? (
          filteredCandidates.map((candidate) => {
            const clip = clipByCandidate.get(candidate.id);
            const isCuttingThis = renderingCandidateId === candidate.id;
            const igPost = instagramPostByCandidate.get(candidate.id);
            const isPublishingThis =
              publishingCandidateId === candidate.id || igPost?.status === "publishing";

            return (
              <MomentCard
                key={candidate.id}
                candidate={candidate}
                clip={clip}
                igPost={igPost}
                isCuttingThis={isCuttingThis}
                isPublishingThis={isPublishingThis}
                appSection={appSection}
                reframeMode={reframeMode}
                sourcePath={detail.project.sourcePath}
                formatTime={formatTime}
                onToggleSelect={toggleCandidate}
                onOpenSocialKit={handleOpenSocialKit}
                onCutCandidate={cutCandidate}
                onOpenFolder={openFolder}
                onPublishToInstagram={handlePublishToInstagram}
                hasFfmpeg={hasFfmpeg}
                isBusy={busy !== "idle"}
              />
            );
          })
        ) : detail.candidates.length > 0 ? (
          <div className="empty-panel-state">
            <Filter size={28} />
            <p>No candidates match the active tab filter.</p>
          </div>
        ) : (
          <div className="empty-panel-state">
            <Sparkles size={36} />
            <h4>No Viral Moments Detected</h4>
            <p>Click "Find Moments" to use AI to locate high-retention viral segments.</p>
            <div className="empty-state-buttons">
              <button
                className="studio-btn primary"
                onClick={onFindMoments}
                disabled={busy !== "idle" || !detail.transcript || !canUseActiveLlm}
              >
                {busy === "moments" ? <Loader2 className="spin" size={14} /> : <Sparkles size={14} />}
                Find Viral Moments
              </button>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
