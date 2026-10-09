import React, { useState, useEffect, useMemo, useRef } from "react";
import {
  Youtube,
  X,
  ExternalLink,
  Loader2,
  Check,
  BadgeCheck,
  AlertTriangle,
  Play,
  Copy,
  Plus,
  RefreshCw,
  Sparkles,
  Key,
  Flame,
  MessageCircle,
  ThumbsUp,
  ThumbsDown,
  Share2,
  Disc3,
  Settings,
} from "lucide-react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { AccessibleModal } from "../../components/AccessibleModal";
import type {
  Candidate,
  Clip,
  YouTubePost,
  EnvironmentStatus,
  SocialKit,
  CaptionOption,
} from "../../types";

interface YouTubePublishModalProps {
  isOpen: boolean;
  onClose: () => void;
  candidate: Candidate | null;
  clip?: Clip;
  environment: EnvironmentStatus | null;
  // Account credentials
  clientId: string;
  setClientId: (id: string) => void;
  clientSecret: string;
  setClientSecret: (secret: string) => void;
  refreshToken: string;
  setRefreshToken: (token: string) => void;
  onSaveCredentials?: () => Promise<void>;
  savingCredentials?: boolean;
  onTestConnection: () => Promise<void>;
  testing: boolean;
  status: { success: boolean; message: string } | null;
  // Publishing actions
  onPublish: (
    candidateId: string,
    titleOverride?: string,
    descriptionOverride?: string,
    privacyStatus?: string,
    tagsOverride?: string[]
  ) => Promise<YouTubePost | void>;
  publishing: boolean;
  existingPost?: YouTubePost;
  onOpenExternal: (url: string) => void;
  onShowToast: (msg: string) => void;
  onRegenerateKit?: (candidateId: string) => Promise<SocialKit | void>;
}

export function YouTubePublishModal({
  isOpen,
  onClose,
  candidate,
  clip,
  environment,
  clientId,
  setClientId,
  clientSecret,
  setClientSecret,
  refreshToken,
  setRefreshToken,
  onSaveCredentials,
  savingCredentials = false,
  onTestConnection,
  testing,
  status,
  onPublish,
  publishing,
  existingPost,
  onOpenExternal,
  onShowToast,
  onRegenerateKit,
}: YouTubePublishModalProps) {
  const [activeTab, setActiveTab] = useState<"studio" | "settings">("studio");
  const [selectedStyle, setSelectedStyle] = useState<string>("hook_focused");
  const [activeTitle, setActiveTitle] = useState<string>("");
  const [editedDescriptions, setEditedDescriptions] = useState<Record<string, string>>({});
  const [activeDescription, setActiveDescription] = useState<string>("");
  const [tags, setTags] = useState<string[]>([]);
  const [newTagInput, setNewTagInput] = useState<string>("");
  const [privacyStatus, setPrivacyStatus] = useState<string>("public");
  const [isRegenerating, setIsRegenerating] = useState<boolean>(false);
  const [isCopiedTitle, setIsCopiedTitle] = useState<boolean>(false);
  const [isCopiedDesc, setIsCopiedDesc] = useState<boolean>(false);
  const [publishStep, setPublishStep] = useState<number>(0);
  const [localSavingCreds, setLocalSavingCreds] = useState<boolean>(false);
  const [videoPlaying, setVideoPlaying] = useState<boolean>(false);

  const videoRef = useRef<HTMLVideoElement>(null);

  // Fallback caption options when social kit options aren't present
  const defaultOptions: CaptionOption[] = useMemo(() => {
    if (!candidate) return [];
    return [
      {
        style: "hook_focused",
        title: "Hook-Focused",
        hook: candidate.hook,
        text: `${candidate.hook}\n\nHere is what you need to know about this moment.`,
      },
      {
        style: "conversational",
        title: "Natural & Conversational",
        hook: "Real perspective on this",
        text: `I look at this for a very specific reason: ${candidate.rationale}`,
      },
      {
        style: "insight_focused",
        title: "Key Takeaway",
        hook: "Key insight from this clip",
        text: `The single biggest takeaway: ${candidate.rationale}\n\nSave this Short for later.`,
      },
    ];
  }, [candidate]);

  const activeKit = candidate?.socialKit;
  const availableOptions: CaptionOption[] =
    activeKit?.captionOptions && activeKit.captionOptions.length > 0
      ? activeKit.captionOptions
      : defaultOptions;

  // Initialize title, description, and tags from Candidate AI Social Kit
  useEffect(() => {
    if (!candidate) return;

    // 1. Title initialization: prefer Social Kit title, then first option hook, then candidate hook
    let initTitle = candidate.hook;
    if (activeKit?.titles && activeKit.titles.length > 0) {
      initTitle = activeKit.titles[0];
    } else if (activeKit?.captionOptions && activeKit.captionOptions.length > 0) {
      initTitle = activeKit.captionOptions[0].hook;
    }
    // Append #Shorts if not present and room allows
    if (!initTitle.toLowerCase().includes("#shorts") && initTitle.length <= 92) {
      initTitle = `${initTitle} #Shorts`;
    }
    setActiveTitle(initTitle);

    // 2. Tags initialization from Social Kit hashtags
    let initialTags: string[] = ["Shorts", "ClipOn"];
    if (activeKit?.hashtags && activeKit.hashtags.length > 0) {
      const sanitized = activeKit.hashtags.map((h) =>
        h.startsWith("#") ? h.slice(1).trim() : h.trim()
      );
      initialTags = Array.from(new Set([...initialTags, ...sanitized]));
    }
    setTags(initialTags);

    // 3. Description initialization
    const currentOption =
      availableOptions.find((o) => o.style === selectedStyle) || availableOptions[0];
    const initialText = currentOption?.text || candidate.rationale || "";
    const tagsString = initialTags.map((t) => `#${t}`).join(" ");
    const fullDesc = `${initialText}\n\n${tagsString}`.trim();

    setActiveDescription(fullDesc);
    setEditedDescriptions({
      [selectedStyle]: fullDesc,
    });
  }, [candidate, activeKit]);

  // Switch between AI caption angles while preserving custom user edits
  function handleSelectStyle(styleKey: string) {
    if (styleKey === selectedStyle) return;

    // Save current description in cache
    const updatedEdits = {
      ...editedDescriptions,
      [selectedStyle]: activeDescription,
    };
    setEditedDescriptions(updatedEdits);

    setSelectedStyle(styleKey);

    // Check if user already edited this style
    if (updatedEdits[styleKey]) {
      setActiveDescription(updatedEdits[styleKey]);
    } else {
      const opt = availableOptions.find((o) => o.style === styleKey);
      const text = opt?.text || candidate?.rationale || "";
      const tagsString = tags.map((t) => `#${t}`).join(" ");
      const newDesc = `${text}\n\n${tagsString}`.trim();
      setActiveDescription(newDesc);
      setEditedDescriptions((prev) => ({ ...prev, [styleKey]: newDesc }));
    }
  }

  // Handle manual description text edits
  function handleDescriptionChange(val: string) {
    setActiveDescription(val);
    setEditedDescriptions((prev) => ({
      ...prev,
      [selectedStyle]: val,
    }));
  }

  // Handle title pill selection from AI titles
  function handlePickTitle(title: string) {
    let formatted = title;
    if (!formatted.toLowerCase().includes("#shorts") && formatted.length <= 92) {
      formatted = `${formatted} #Shorts`;
    }
    setActiveTitle(formatted);
  }

  // Add a new tag chip
  function handleAddTag() {
    const trimmed = newTagInput.trim().replace(/^#/, "");
    if (!trimmed) return;
    if (!tags.some((t) => t.toLowerCase() === trimmed.toLowerCase())) {
      setTags((prev) => [...prev, trimmed]);
      setNewTagInput("");
    }
  }

  // Remove tag chip
  function handleRemoveTag(tagToRemove: string) {
    setTags((prev) => prev.filter((t) => t !== tagToRemove));
  }

  // Copy title to clipboard
  async function handleCopyTitle() {
    try {
      await navigator.clipboard.writeText(activeTitle);
      setIsCopiedTitle(true);
      setTimeout(() => setIsCopiedTitle(false), 2000);
      onShowToast("Title copied to clipboard!");
    } catch {
      onShowToast("Failed to copy title");
    }
  }

  // Copy full description to clipboard
  async function handleCopyDescription() {
    try {
      await navigator.clipboard.writeText(activeDescription);
      setIsCopiedDesc(true);
      setTimeout(() => setIsCopiedDesc(false), 2000);
      onShowToast("Description copied to clipboard!");
    } catch {
      onShowToast("Failed to copy description");
    }
  }

  // Regenerate Social Kit on the fly
  async function handleRegenerate() {
    if (!candidate || !onRegenerateKit) return;
    setIsRegenerating(true);
    try {
      const newKit = await onRegenerateKit(candidate.id);
      if (newKit) {
        onShowToast("✨ AI Social Kit regenerated with fresh titles and angles!");
      }
    } catch (err) {
      onShowToast(`Regeneration failed: ${String(err)}`);
    } finally {
      setIsRegenerating(false);
    }
  }

  // Stepper advancement simulation during publishing
  useEffect(() => {
    let interval: ReturnType<typeof setInterval> | null = null;
    if (publishing) {
      setPublishStep(1);
      interval = setInterval(() => {
        setPublishStep((prev) => (prev < 4 ? prev + 1 : prev));
      }, 3500);
    } else {
      setPublishStep(0);
    }
    return () => {
      if (interval) clearInterval(interval);
    };
  }, [publishing]);

  // Video preview URL
  const videoSrc = useMemo(() => {
    if (clip?.outputPath) {
      return convertFileSrc(clip.outputPath);
    }
    return null;
  }, [clip]);

  // Handle direct publish action
  async function handlePublishClick() {
    if (!candidate) return;
    if (!activeTitle.trim()) {
      onShowToast("Please enter a title for your Short.");
      return;
    }

    try {
      await onPublish(
        candidate.id,
        activeTitle.trim(),
        activeDescription.trim(),
        privacyStatus,
        tags
      );
      setPublishStep(5);
    } catch (err) {
      setPublishStep(0);
      onShowToast(`Publishing failed: ${String(err)}`);
    }
  }

  // Save credentials inline
  async function handleSaveCredentialsInline() {
    if (onSaveCredentials) {
      setLocalSavingCreds(true);
      try {
        await onSaveCredentials();
        onShowToast("YouTube OAuth2 credentials saved securely!");
        if (candidate) {
          setActiveTab("studio");
        }
      } catch (err) {
        onShowToast(`Failed to save: ${String(err)}`);
      } finally {
        setLocalSavingCreds(false);
      }
    }
  }

  if (!isOpen) return null;

  const isConnected = Boolean(
    environment?.hasYoutubeConfig ||
      (clientId.trim() && clientSecret.trim() && refreshToken.trim())
  );

  return (
    <AccessibleModal
      isOpen={isOpen}
      onClose={onClose}
      title="YouTube Shorts Publishing Studio"
      titleId="yt-publishing-studio-title"
      dialogClassName="ig-publishing-modal"
    >
      {/* Studio Header */}
      <div className="modal-header">
        <div className="modal-header-left">
          <div
            className="modal-icon-badge"
            style={{
              background: "linear-gradient(135deg, #ef4444 0%, #dc2626 50%, #991b1b 100%)",
              color: "#ffffff",
              borderColor: "rgba(239, 68, 68, 0.4)",
            }}
          >
            <Youtube size={18} />
          </div>
          <div>
            <h3 id="yt-publishing-studio-title">
              Publish Short • {candidate ? `Clip #${candidate.rank}` : "YouTube Shorts"}
            </h3>
            <p>
              AI Social Kit title generation, 3 caption angles, and direct YouTube Data API v3 upload
            </p>
          </div>
        </div>

        <div className="modal-header-actions">
          {/* Navigation Tabs */}
          <div className="studio-tab-selector">
            <button
              type="button"
              className={`studio-tab-btn ${activeTab === "studio" ? "active" : ""}`}
              onClick={() => setActiveTab("studio")}
            >
              <Sparkles size={13} />
              <span>Publish Studio</span>
            </button>
            <button
              type="button"
              className={`studio-tab-btn ${activeTab === "settings" ? "active" : ""}`}
              onClick={() => setActiveTab("settings")}
            >
              <Settings size={13} />
              <span>OAuth2 Credentials</span>
              {!isConnected && <span className="tab-warning-dot" />}
            </button>
          </div>

          <button className="modal-close-btn" onClick={onClose} aria-label="Close dialog">
            <X size={16} />
          </button>
        </div>
      </div>

      {/* Main Studio Body */}
      <div className="ig-studio-body">
        {activeTab === "studio" ? (
          <div className="ig-studio-grid">
            {/* LEFT COLUMN: AI Social Kit Editor */}
            <div className="ig-studio-left-pane">
              {/* Candidate Info Pill Banner */}
              {candidate && (
                <div className="ig-clip-meta-banner">
                  <div className="clip-meta-info">
                    <span
                      className="clip-rank-pill"
                      style={{
                        background: "rgba(239, 68, 68, 0.18)",
                        color: "#ef4444",
                        borderColor: "rgba(239, 68, 68, 0.3)",
                      }}
                    >
                      CLIP #{candidate.rank}
                    </span>
                    <span className="clip-hook-text" title={candidate.hook}>
                      {candidate.hook}
                    </span>
                  </div>
                  <div className="clip-score-badge">
                    <Flame size={12} />
                    <span>{(candidate.score * 100).toFixed(0)}% Viral Score</span>
                  </div>
                </div>
              )}

              {/* AI Titles Selector */}
              <div className="ig-caption-studio-section">
                <div className="ig-caption-section-header">
                  <div className="caption-section-title-wrap">
                    <label className="caption-section-label">AI Video Title (Under 100 Chars)</label>
                    <span className="caption-helper-text">
                      Click a title option or edit directly. Auto-formats with #Shorts.
                    </span>
                  </div>
                  <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
                    <span
                      style={{
                        fontSize: "11px",
                        color: activeTitle.length > 100 ? "#ef4444" : "var(--text-muted, #94a3b8)",
                        fontVariantNumeric: "tabular-nums",
                      }}
                    >
                      {activeTitle.length} / 100 chars
                    </span>
                    <button
                      type="button"
                      className="action-pill-btn"
                      onClick={handleCopyTitle}
                      title="Copy title"
                    >
                      {isCopiedTitle ? <Check size={11} /> : <Copy size={11} />}
                      <span>{isCopiedTitle ? "Copied" : "Copy"}</span>
                    </button>
                  </div>
                </div>

                {/* AI Title Suggestions Pills */}
                {activeKit?.titles && activeKit.titles.length > 0 && (
                  <div style={{ display: "flex", flexWrap: "wrap", gap: "6px" }}>
                    {activeKit.titles.map((t, idx) => (
                      <button
                        key={idx}
                        type="button"
                        onClick={() => handlePickTitle(t)}
                        className="studio-tab-btn"
                        style={{
                          background:
                            activeTitle.includes(t) || t === activeTitle
                              ? "rgba(239, 68, 68, 0.15)"
                              : "rgba(255, 255, 255, 0.05)",
                          color:
                            activeTitle.includes(t) || t === activeTitle
                              ? "#ef4444"
                              : "var(--text-secondary, #cbd5e1)",
                          borderColor:
                            activeTitle.includes(t) || t === activeTitle
                              ? "rgba(239, 68, 68, 0.3)"
                              : "rgba(255, 255, 255, 0.1)",
                          borderWidth: "1px",
                          borderStyle: "solid",
                          fontSize: "11px",
                          padding: "4px 8px",
                          borderRadius: "6px",
                          cursor: "pointer",
                        }}
                      >
                        <Sparkles size={11} />
                        <span>{t}</span>
                      </button>
                    ))}
                  </div>
                )}

                <input
                  type="text"
                  value={activeTitle}
                  onChange={(e) => setActiveTitle(e.target.value)}
                  maxLength={100}
                  placeholder="Catchy Short Title #Shorts"
                  style={{
                    width: "100%",
                    background: "rgba(0, 0, 0, 0.35)",
                    border: "1px solid rgba(255, 255, 255, 0.12)",
                    borderRadius: "8px",
                    padding: "8px 12px",
                    color: "#ffffff",
                    fontSize: "13px",
                    fontWeight: 600,
                  }}
                />
              </div>

              {/* 3 AI Angles Option Selector */}
              <div className="ig-caption-studio-section">
                <div className="ig-caption-section-header">
                  <div className="caption-section-title-wrap">
                    <label className="caption-section-label">AI Social Kit Angles</label>
                    <span className="caption-helper-text">
                      Select angle to populate description &amp; preview
                    </span>
                  </div>
                  {onRegenerateKit && (
                    <button
                      type="button"
                      className="caption-action-btn secondary"
                      onClick={() => void handleRegenerate()}
                      disabled={isRegenerating || publishing}
                      title="Regenerate Social Kit"
                    >
                      <RefreshCw size={12} className={isRegenerating ? "spin" : ""} />
                      <span>{isRegenerating ? "Generating..." : "Regenerate AI"}</span>
                    </button>
                  )}
                </div>

                <div className="caption-option-tabs">
                  {availableOptions.map((opt) => {
                    const isActive = selectedStyle === opt.style;
                    return (
                      <button
                        key={opt.style}
                        type="button"
                        className={`caption-tab-card ${isActive ? "active" : ""}`}
                        style={
                          isActive
                            ? {
                                background: "rgba(239, 68, 68, 0.08)",
                                borderColor: "rgba(239, 68, 68, 0.45)",
                                boxShadow: "0 0 12px rgba(239, 68, 68, 0.12)",
                              }
                            : {}
                        }
                        onClick={() => handleSelectStyle(opt.style)}
                      >
                        <div className="tab-card-header">
                          <span className="tab-card-title">{opt.title}</span>
                          {isActive && (
                            <span
                              className="tab-active-dot"
                              style={{ background: "#ef4444", boxShadow: "0 0 6px #ef4444" }}
                            />
                          )}
                        </div>
                        <p className="tab-card-hook-preview">"{opt.hook}"</p>
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* Description Content Editor */}
              <div className="ig-caption-studio-section">
                <div className="ig-caption-section-header">
                  <label className="caption-section-label">Video Description</label>
                  <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
                    <span
                      style={{
                        fontSize: "11px",
                        color: "var(--text-muted, #94a3b8)",
                        fontVariantNumeric: "tabular-nums",
                      }}
                    >
                      {activeDescription.length} / 5,000 chars
                    </span>
                    <button
                      type="button"
                      className="action-pill-btn"
                      onClick={handleCopyDescription}
                      title="Copy description"
                    >
                      {isCopiedDesc ? <Check size={11} /> : <Copy size={11} />}
                      <span>{isCopiedDesc ? "Copied" : "Copy"}</span>
                    </button>
                  </div>
                </div>

                <textarea
                  className="caption-textarea"
                  value={activeDescription}
                  onChange={(e) => handleDescriptionChange(e.target.value)}
                  placeholder="Write or edit your YouTube Shorts description..."
                  rows={4}
                  maxLength={5000}
                />
              </div>

              {/* YouTube Tags Chips Manager */}
              <div className="ig-hashtags-section">
                <div className="ig-caption-section-header">
                  <label className="caption-section-label">Target Tags ({tags.length})</label>
                  <span className="caption-helper-text">
                    Tags enhance search discoverability on YouTube Shorts
                  </span>
                </div>

                <div className="hashtag-chips-container">
                  {tags.map((tag) => (
                    <span key={tag} className="ig-hashtag-chip">
                      <span>#{tag}</span>
                      <button
                        type="button"
                        className="chip-remove-btn"
                        onClick={() => handleRemoveTag(tag)}
                        title={`Remove #${tag}`}
                      >
                        <X size={10} />
                      </button>
                    </span>
                  ))}

                  <div className="hashtag-input-chip">
                    <span style={{ fontSize: "11px", color: "rgba(255,255,255,0.4)" }}>#</span>
                    <input
                      type="text"
                      className="hashtag-mini-input"
                      placeholder="add tag"
                      value={newTagInput}
                      onChange={(e) => setNewTagInput(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          handleAddTag();
                        }
                      }}
                    />
                    <button
                      type="button"
                      className="chip-add-btn"
                      onClick={handleAddTag}
                      disabled={!newTagInput.trim()}
                    >
                      <Plus size={11} />
                    </button>
                  </div>
                </div>
              </div>

              {/* Privacy Setting & Publishing Stepper */}
              <div style={{ display: "flex", gap: "14px", alignItems: "center" }}>
                <div style={{ flex: 1 }}>
                  <label className="caption-section-label" style={{ marginBottom: "6px", display: "block" }}>
                    Privacy Setting
                  </label>
                  <select
                    value={privacyStatus}
                    onChange={(e) => setPrivacyStatus(e.target.value)}
                    style={{
                      width: "100%",
                      background: "rgba(0, 0, 0, 0.35)",
                      border: "1px solid rgba(255, 255, 255, 0.12)",
                      borderRadius: "8px",
                      padding: "8px 12px",
                      color: "#ffffff",
                      fontSize: "12px",
                      cursor: "pointer",
                    }}
                  >
                    <option value="public">🌐 Public (Instant Live Short)</option>
                    <option value="unlisted">👁️ Unlisted (Review via Direct Link)</option>
                    <option value="private">🔒 Private (Only You)</option>
                  </select>
                </div>
              </div>

              {/* Publishing Stepper Progress Bar */}
              {publishing && (
                <div className="publishing-stepper-box">
                  <div className="stepper-header">
                    <span className="stepper-title">Uploading to YouTube Shorts</span>
                    <span className="stepper-step-count">Step {publishStep} of 4</span>
                  </div>
                  <div className="stepper-track">
                    <div
                      className="stepper-fill"
                      style={{
                        width: `${(publishStep / 4) * 100}%`,
                        background: "linear-gradient(90deg, #ef4444, #dc2626)",
                      }}
                    />
                  </div>
                  <div className="stepper-labels">
                    <span className={publishStep >= 1 ? "active" : ""}>Validate</span>
                    <span className={publishStep >= 2 ? "active" : ""}>Refresh Token</span>
                    <span className={publishStep >= 3 ? "active" : ""}>Resumable Session</span>
                    <span className={publishStep >= 4 ? "active" : ""}>Upload Bytes</span>
                  </div>
                </div>
              )}

              {/* Post Success or Existing Post Banner */}
              {existingPost && existingPost.status === "published" && (
                <div
                  className="post-status-banner success"
                  style={{
                    background: "rgba(16, 185, 129, 0.12)",
                    borderColor: "rgba(16, 185, 129, 0.35)",
                  }}
                >
                  <BadgeCheck size={18} color="#10b981" />
                  <div className="banner-content">
                    <strong>Published on YouTube Shorts!</strong>
                    <span>Your video is live on YouTube.</span>
                  </div>
                  {existingPost.videoUrl && (
                    <button
                      type="button"
                      className="studio-btn secondary small"
                      onClick={() => onOpenExternal(existingPost.videoUrl!)}
                    >
                      <ExternalLink size={12} />
                      <span>View Short</span>
                    </button>
                  )}
                </div>
              )}
            </div>

            {/* RIGHT COLUMN: Realistic YouTube Shorts Phone Mockup */}
            <div className="ig-studio-right-pane">
              <div className="reels-mockup-frame">
                {/* Phone Notch */}
                <div className="phone-notch">
                  <div className="notch-speaker" />
                </div>

                {/* Screen Content */}
                <div className="phone-screen-content">
                  {videoSrc ? (
                    <video
                      ref={videoRef}
                      className="reel-video-element"
                      src={videoSrc}
                      loop
                      playsInline
                      muted
                      onClick={() => {
                        if (videoRef.current) {
                          if (videoPlaying) {
                            videoRef.current.pause();
                            setVideoPlaying(false);
                          } else {
                            void videoRef.current.play();
                            setVideoPlaying(true);
                          }
                        }
                      }}
                    />
                  ) : (
                    <div className="reel-poster-fallback">
                      <div className="fallback-play-circle">
                        <Play size={22} fill="currentColor" />
                      </div>
                      <span style={{ fontSize: "12px", fontWeight: 600 }}>9:16 Shorts Preview</span>
                      <span style={{ fontSize: "10px", marginTop: "4px" }}>
                        Clip renders automatically on upload
                      </span>
                    </div>
                  )}

                  {/* YouTube Shorts UI Overlays */}
                  <div className="reels-overlay-layer">
                    {/* Top Header */}
                    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                      <span style={{ fontSize: "12px", fontWeight: 700, color: "#fff", textShadow: "0 1px 3px rgba(0,0,0,0.8)" }}>
                        Shorts
                      </span>
                      <div style={{ display: "flex", gap: "10px" }}>
                        <span style={{ fontSize: "10px", color: "rgba(255,255,255,0.7)" }}>
                          {candidate ? `${(candidate.endSec - candidate.startSec).toFixed(0)}s` : "0:30"}
                        </span>
                      </div>
                    </div>

                    {/* Right-Side Action Rail */}
                    <div className="reels-actions-col">
                      <div className="reels-action-item">
                        <div className="action-circle">
                          <ThumbsUp size={19} />
                        </div>
                        <span>14K</span>
                      </div>
                      <div className="reels-action-item">
                        <div className="action-circle">
                          <ThumbsDown size={19} />
                        </div>
                        <span>Dislike</span>
                      </div>
                      <div className="reels-action-item">
                        <div className="action-circle">
                          <MessageCircle size={19} />
                        </div>
                        <span>248</span>
                      </div>
                      <div className="reels-action-item">
                        <div className="action-circle">
                          <Share2 size={19} />
                        </div>
                        <span>Share</span>
                      </div>
                      <div className="reels-action-item">
                        <div className="action-circle">
                          <Disc3 size={19} className="spin" />
                        </div>
                        <span>Sound</span>
                      </div>
                    </div>

                    {/* Bottom Metadata & Dynamic Title */}
                    <div className="reels-bottom-info">
                      <div className="reels-creator-row">
                        <div
                          className="creator-avatar"
                          style={{
                            background: "linear-gradient(135deg, #ef4444, #b91c1c)",
                          }}
                        >
                          YT
                        </div>
                        <span className="creator-handle">@YourChannel</span>
                        <span
                          style={{
                            background: "#ffffff",
                            color: "#000000",
                            fontSize: "10px",
                            fontWeight: 700,
                            padding: "2px 8px",
                            borderRadius: "12px",
                          }}
                        >
                          Subscribe
                        </span>
                      </div>

                      <div className="reels-caption-box">
                        <p
                          className="reels-caption-text"
                          style={{ fontWeight: 600, fontSize: "12px" }}
                        >
                          {activeTitle || "Catchy Title #Shorts"}
                        </p>
                      </div>

                      {tags.length > 0 && (
                        <span
                          className="reels-tags-text"
                          style={{ color: "#ef4444", fontWeight: 600 }}
                        >
                          {tags.slice(0, 3).map((t) => `#${t}`).join(" ")}
                        </span>
                      )}
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        ) : (
          /* TAB 2: YouTube OAuth2 Settings */
          <div className="settings-form-stack" style={{ maxWidth: "600px", margin: "0 auto" }}>
            <div className="youtube-banner-box">
              <strong className="youtube-banner-title">Google Cloud OAuth2 Configuration</strong>
              Enter your Google Cloud OAuth2 Client ID, Client Secret, and Refresh Token. ClipOn
              securely stores these in your local restricted keystore (0600 permissions) and manages
              automatic token refreshment.
            </div>

            <div className="settings-field-group">
              <label
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                }}
              >
                <span>OAuth2 Client ID</span>
                <button
                  type="button"
                  className="action-pill-btn"
                  onClick={() => onOpenExternal("https://console.cloud.google.com/apis/credentials")}
                >
                  <ExternalLink size={10} />
                  <span>Google Cloud Console</span>
                </button>
              </label>
              <input
                type="text"
                value={clientId}
                onChange={(e) => setClientId(e.target.value)}
                placeholder="e.g. 1234567890-xxx.apps.googleusercontent.com"
              />
            </div>

            <div className="settings-field-group">
              <label>OAuth2 Client Secret</label>
              <input
                type="password"
                value={clientSecret}
                onChange={(e) => setClientSecret(e.target.value)}
                placeholder="GOCSPX-..."
              />
            </div>

            <div className="settings-field-group">
              <label>OAuth2 Refresh Token</label>
              <input
                type="password"
                value={refreshToken}
                onChange={(e) => setRefreshToken(e.target.value)}
                placeholder="1//04..."
              />
              <span className="folder-hint">
                Requires scope: https://www.googleapis.com/auth/youtube.upload
              </span>
            </div>

            <div style={{ display: "flex", gap: "8px", marginTop: "12px" }}>
              <button
                type="button"
                className="studio-btn secondary small"
                onClick={() => void onTestConnection()}
                disabled={testing}
              >
                {testing ? <Loader2 className="spin" size={13} /> : <Check size={13} />}
                <span>{testing ? "Testing..." : "Test Connection"}</span>
              </button>

              {onSaveCredentials && (
                <button
                  type="button"
                  className="studio-btn primary small"
                  onClick={() => void handleSaveCredentialsInline()}
                  disabled={savingCredentials || localSavingCreds}
                >
                  {savingCredentials || localSavingCreds ? (
                    <Loader2 className="spin" size={13} />
                  ) : (
                    <Key size={13} />
                  )}
                  <span>
                    {savingCredentials || localSavingCreds ? "Saving..." : "Save Credentials"}
                  </span>
                </button>
              )}
            </div>

            {status && (
              <div className={`connection-status-banner ${status.success ? "success" : "error"}`}>
                {status.success ? <BadgeCheck size={16} /> : <AlertTriangle size={16} />}
                <span>{status.message}</span>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Studio Footer */}
      <div className="modal-footer">
        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          {isConnected ? (
            <span
              className="connection-pill active"
              style={{
                background: "rgba(16, 185, 129, 0.15)",
                color: "#10b981",
                border: "1px solid rgba(16, 185, 129, 0.3)",
                fontSize: "11px",
                padding: "3px 8px",
                borderRadius: "9999px",
              }}
            >
              ● YouTube OAuth2 Connected
            </span>
          ) : (
            <span
              className="connection-pill inactive"
              style={{
                background: "rgba(245, 158, 11, 0.15)",
                color: "#f59e0b",
                border: "1px solid rgba(245, 158, 11, 0.3)",
                fontSize: "11px",
                padding: "3px 8px",
                borderRadius: "9999px",
              }}
            >
              ⚠️ Setup Required in Credentials Tab
            </span>
          )}
        </div>

        <div style={{ display: "flex", gap: "10px" }}>
          <button className="studio-btn secondary" onClick={onClose} disabled={publishing}>
            Cancel
          </button>

          <button
            className="studio-btn primary"
            style={{
              background: "linear-gradient(135deg, #ef4444 0%, #dc2626 100%)",
              borderColor: "rgba(239, 68, 68, 0.4)",
              color: "#ffffff",
              boxShadow: "0 4px 14px rgba(239, 68, 68, 0.3)",
            }}
            onClick={() => void handlePublishClick()}
            disabled={publishing || !isConnected}
          >
            {publishing ? (
              <Loader2 className="spin" size={14} />
            ) : (
              <Youtube size={14} />
            )}
            <span>
              {publishing ? "Uploading Short..." : "Post Short to YouTube"}
            </span>
          </button>
        </div>
      </div>
    </AccessibleModal>
  );
}
