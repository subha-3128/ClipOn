import React, { useState, useEffect, useMemo, useRef } from "react";
import {
  Instagram,
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
  Heart,
  Music,
  Share2,
  Settings,
} from "lucide-react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { AccessibleModal } from "../../components/AccessibleModal";
import type {
  Candidate,
  Clip,
  InstagramPost,
  EnvironmentStatus,
  SocialKit,
  CaptionOption,
} from "../../types";

interface InstagramPublishModalProps {
  isOpen: boolean;
  onClose: () => void;
  candidate: Candidate | null;
  clip?: Clip;
  environment: EnvironmentStatus | null;
  // Account credentials
  accountId: string;
  setAccountId: (id: string) => void;
  accessToken: string;
  setAccessToken: (token: string) => void;
  provider: "graph_api" | "webhook";
  setProvider: (p: "graph_api" | "webhook") => void;
  webhookUrl: string;
  setWebhookUrl: (u: string) => void;
  onSaveCredentials?: () => Promise<void>;
  saving?: boolean;
  onTestConnection: () => Promise<void>;
  testing: boolean;
  testStatus: { success: boolean; message: string } | null;
  // Publishing actions
  onPublish: (candidateId: string, captionOverride: string) => Promise<InstagramPost | void>;
  publishing: boolean;
  existingPost?: InstagramPost;
  onOpenExternal: (url: string) => void;
  onShowToast: (msg: string) => void;
  onRegenerateKit?: (candidateId: string) => Promise<SocialKit | void>;
}

export function InstagramPublishModal({
  isOpen,
  onClose,
  candidate,
  clip,
  environment,
  accountId,
  setAccountId,
  accessToken,
  setAccessToken,
  provider,
  setProvider,
  webhookUrl,
  setWebhookUrl,
  onSaveCredentials,
  saving = false,
  onTestConnection,
  testing,
  testStatus,
  onPublish,
  publishing,
  existingPost,
  onOpenExternal,
  onShowToast,
  onRegenerateKit,
}: InstagramPublishModalProps) {
  const [activeTab, setActiveTab] = useState<"studio" | "settings">("studio");
  const [selectedStyle, setSelectedStyle] = useState<string>("hook_focused");
  const [editedCaptions, setEditedCaptions] = useState<Record<string, string>>({});
  const [activeCaption, setActiveCaption] = useState<string>("");
  const [hashtags, setHashtags] = useState<string[]>([]);
  const [newTagInput, setNewTagInput] = useState<string>("");
  const [isRegenerating, setIsRegenerating] = useState<boolean>(false);
  const [isCopied, setIsCopied] = useState<boolean>(false);
  const [publishStep, setPublishStep] = useState<number>(0);
  const [localSavingCreds, setLocalSavingCreds] = useState<boolean>(false);
  const [videoPlaying, setVideoPlaying] = useState<boolean>(false);

  const videoRef = useRef<HTMLVideoElement>(null);

  // Check if credentials are configured
  const isAccountConfigured = useMemo(() => {
    if (provider === "graph_api") {
      const hasId = Boolean(accountId.trim() || environment?.instagramAccountId);
      const hasToken = Boolean(accessToken.trim() || environment?.hasInstagramToken);
      return hasId && hasToken;
    }
    return Boolean(webhookUrl.trim());
  }, [provider, accountId, accessToken, webhookUrl, environment]);

  // Fallback options if socialKit is not loaded
  const captionOptions: CaptionOption[] = useMemo(() => {
    if (candidate?.socialKit?.captionOptions && candidate.socialKit.captionOptions.length > 0) {
      return candidate.socialKit.captionOptions;
    }
    const cleanHook = candidate?.hook?.trim() || "Viral Reel Moment";
    return [
      {
        style: "hook_focused",
        title: "Hook-Focused",
        hook: cleanHook,
        text: `${cleanHook}\n\nHere is why this insight matters.\n\nWhat's your take on this?`,
      },
      {
        style: "conversational",
        title: "Natural & Conversational",
        hook: `A thought on: ${cleanHook}`,
        text: `This part of the conversation really stood out to me: "${cleanHook}"\n\nHow do you look at this in your daily workflow?`,
      },
      {
        style: "insight_focused",
        title: "Key Takeaway",
        hook: `Key insight: ${cleanHook}`,
        text: `Key takeaway from this clip:\n\n• ${cleanHook}\n• Keep it simple and focused.\n\nSave this reminder for later.`,
      },
    ];
  }, [candidate]);

  // Synchronize initial caption and hashtags when candidate changes or modal opens
  useEffect(() => {
    if (isOpen && candidate) {
      const initialTags = candidate.socialKit?.hashtags || [
        "#ReelsOriginal",
        "#CreatorTips",
        "#Shorts",
      ];
      setHashtags(initialTags);

      const defaultOpt = captionOptions[0];
      const initialText = defaultOpt?.text || candidate.hook || "";
      setSelectedStyle(defaultOpt?.style || "hook_focused");
      setActiveCaption(initialText);
      setEditedCaptions({
        [defaultOpt?.style || "hook_focused"]: initialText,
      });

      if (!isAccountConfigured) {
        setActiveTab("settings");
      } else {
        setActiveTab("studio");
      }
    }
  }, [isOpen, candidate?.id, isAccountConfigured]);

  // Handle switching caption options while preserving user edits
  function handleSelectStyle(newStyle: string) {
    if (newStyle === selectedStyle) return;

    // Cache current edit for current style
    setEditedCaptions((prev) => ({
      ...prev,
      [selectedStyle]: activeCaption,
    }));

    // Check if new style has cached edit or fallback to original generated text
    const cached = editedCaptions[newStyle];
    if (cached !== undefined) {
      setActiveCaption(cached);
    } else {
      const found = captionOptions.find((o) => o.style === newStyle);
      const targetText = found ? found.text : "";
      setActiveCaption(targetText);
      setEditedCaptions((prev) => ({
        ...prev,
        [newStyle]: targetText,
      }));
    }

    setSelectedStyle(newStyle);
  }

  // Handle caption text change
  function handleCaptionChange(text: string) {
    setActiveCaption(text);
    setEditedCaptions((prev) => ({
      ...prev,
      [selectedStyle]: text,
    }));
  }

  // Add custom hashtag
  function handleAddHashtag() {
    let clean = newTagInput.trim().replace(/\s+/g, "");
    if (!clean) return;
    if (!clean.startsWith("#")) {
      clean = `#${clean}`;
    }
    if (!hashtags.includes(clean)) {
      setHashtags((prev) => [...prev, clean]);
    }
    setNewTagInput("");
  }

  // Remove hashtag
  function handleRemoveHashtag(tagToRemove: string) {
    setHashtags((prev) => prev.filter((t) => t !== tagToRemove));
  }

  // Copy full package
  function handleCopyFullCaption() {
    const full = `${activeCaption.trim()}\n\n${hashtags.join(" ")}`;
    navigator.clipboard.writeText(full);
    setIsCopied(true);
    onShowToast("Caption & Hashtags copied to clipboard!");
    setTimeout(() => setIsCopied(false), 2000);
  }

  // Regenerate AI Captions without touching video
  async function handleRegenerateCaptions() {
    if (!candidate || !onRegenerateKit) return;
    setIsRegenerating(true);
    try {
      const freshKit = await onRegenerateKit(candidate.id);
      if (freshKit && freshKit.captionOptions && freshKit.captionOptions.length > 0) {
        setHashtags(freshKit.hashtags);
        const opt = freshKit.captionOptions[0];
        setSelectedStyle(opt.style);
        setActiveCaption(opt.text);
        setEditedCaptions({
          [opt.style]: opt.text,
        });
        onShowToast("Fresh AI captions generated!");
      }
    } catch (e) {
      onShowToast(`Failed to regenerate captions: ${String(e)}`);
    } finally {
      setIsRegenerating(false);
    }
  }

  // Publish flow with multi-stage stepper
  async function handlePublishToInstagram() {
    if (!candidate) return;
    if (!isAccountConfigured) {
      setActiveTab("settings");
      onShowToast("Please configure your Instagram credentials first");
      return;
    }

    setPublishStep(1);
    const stepTimer = setInterval(() => {
      setPublishStep((prev) => (prev < 4 ? prev + 1 : prev));
    }, 2800);

    try {
      const fullPostCaption = `${activeCaption.trim()}\n\n${hashtags.join(" ")}`;
      await onPublish(candidate.id, fullPostCaption);
      setPublishStep(5);
    } catch {
      setPublishStep(0);
    } finally {
      clearInterval(stepTimer);
    }
  }

  // Save credentials inline
  async function handleSaveCredentialsInline() {
    if (onSaveCredentials) {
      setLocalSavingCreds(true);
      try {
        await onSaveCredentials();
        onShowToast("Instagram credentials saved securely!");
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

  const videoSrc = clip?.outputPath ? convertFileSrc(clip.outputPath) : null;
  const isPublished = existingPost?.status === "published";
  const isPublishFailed = existingPost?.status === "failed";
  const postUrl = existingPost?.postUrl;

  return (
    <AccessibleModal
      isOpen={isOpen}
      onClose={onClose}
      title="Instagram Reels Publishing Studio"
      titleId="ig-publish-studio-title"
      dialogClassName="settings-modal ig-publishing-modal"
    >
      {/* Modal Top Header */}
      <div className="modal-header">
        <div className="modal-header-left">
          <div className="modal-icon-badge instagram-gradient-badge">
            <Instagram size={18} />
          </div>
          <div>
            <h3 id="ig-publish-studio-title">
              {candidate
                ? `Publish Reel • Clip #${candidate.rank}`
                : "Instagram Reels Publishing Studio"}
            </h3>
            <p>
              AI-grounded captions, authentic Reels preview, and direct Meta Graph API publishing
            </p>
          </div>
        </div>

        <div className="modal-header-actions">
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
              <span>Connection</span>
              {!isAccountConfigured && <span className="tab-warning-dot" />}
            </button>
          </div>

          <button
            className="modal-close-btn"
            onClick={onClose}
            aria-label="Close dialog"
          >
            <X size={16} />
          </button>
        </div>
      </div>

      {/* Modal Body */}
      <div className="modal-body ig-studio-body">
        {activeTab === "settings" ? (
          /* ================= Connection & Setup View ================= */
          <div className="ig-settings-view">
            <div className="instagram-banner-box">
              <strong className="instagram-banner-title">
                Meta Graph API Direct Publishing Configuration
              </strong>
              <p style={{ margin: "4px 0 0 0", fontSize: "12px", opacity: 0.9 }}>
                Direct Reel uploads require a <strong>Meta Page Access Token</strong> (starts with <code>EAA...</code>) linked to your Instagram Professional (Creator or Business) account.
              </p>
              <div
                style={{
                  marginTop: "6px",
                  fontSize: "11px",
                  opacity: "0.85",
                  display: "flex",
                  flexDirection: "column",
                  gap: "2px",
                }}
              >
                <span>1. Ensure your Instagram account is set to Professional in Instagram App settings.</span>
                <span>2. Connect your Instagram account to a Facebook Page in Meta Business Suite.</span>
                <span>3. In Meta Graph API Explorer, select your Page to generate an <code>EAA...</code> Page Access Token.</span>
                <span>4. Required scopes: <code>instagram_basic</code>, <code>instagram_content_publish</code>.</span>
              </div>
            </div>

            <div className="settings-field-group">
              <label className="settings-field-label">
                <span>Publishing Method</span>
              </label>
              <div className="settings-option-grid col-2">
                <button
                  type="button"
                  className={`settings-option-card compact ${
                    provider === "graph_api" ? "selected" : ""
                  }`}
                  onClick={() => setProvider("graph_api")}
                >
                  <div className="option-title">Meta Graph API (Direct)</div>
                  <div className="option-desc">
                    Official direct upload to Meta's rupload servers.
                  </div>
                </button>
                <button
                  type="button"
                  className={`settings-option-card compact ${
                    provider === "webhook" ? "selected" : ""
                  }`}
                  onClick={() => setProvider("webhook")}
                >
                  <div className="option-title">Automation Webhook</div>
                  <div className="option-desc">
                    Dispatch video payload to Zapier, Make, n8n, or custom server.
                  </div>
                </button>
              </div>
            </div>

            {provider === "graph_api" ? (
              <div className="settings-form-stack">
                <div className="settings-field-group">
                  <div className="settings-field-header">
                    <label htmlFor="ig-modal-account-id" className="settings-field-label">
                      <span>Instagram Business Account ID</span>
                    </label>
                    <button
                      type="button"
                      className="action-pill-btn"
                      onClick={() =>
                        onOpenExternal("https://developers.facebook.com/tools/explorer/")
                      }
                      title="Open Meta Graph API Explorer"
                    >
                      <ExternalLink size={10} />
                      <span>Graph Explorer</span>
                    </button>
                  </div>
                  <input
                    id="ig-modal-account-id"
                    type="text"
                    value={accountId}
                    onChange={(e) => setAccountId(e.target.value)}
                    placeholder="e.g. 17841400000000000"
                    className="settings-input"
                  />
                  <p className="settings-field-desc">
                    Found in Meta Business Suite or via Graph Explorer <code>GET /me/accounts</code>.
                  </p>
                </div>

                <div className="settings-field-group">
                  <label htmlFor="ig-modal-access-token" className="settings-field-label">
                    <span>Meta Page Access Token (starts with EAA...)</span>
                  </label>
                  <input
                    id="ig-modal-access-token"
                    type="password"
                    value={accessToken}
                    onChange={(e) => setAccessToken(e.target.value)}
                    placeholder={
                      environment?.hasInstagramToken
                        ? "•••••••••••••••• (Saved in Keystore)"
                        : "EAAG... (Page Access Token with instagram_content_publish)"
                    }
                    className="settings-input"
                  />
                  {accessToken.trim().startsWith("IGA") ||
                  accessToken.trim().startsWith("IGQ") ? (
                    <div className="connection-status-banner error" style={{ marginTop: "6px" }}>
                      <AlertTriangle size={14} style={{ flexShrink: 0 }} />
                      <span>
                        <strong>Instagram User Token detected ({accessToken.trim().slice(0, 4)}...).</strong> Direct Reel uploads from desktop require a <strong>Meta Page Access Token</strong> (starts with <code>EAA...</code>). In Graph API Explorer, select your Facebook Page under <em>User or Page</em>.
                      </span>
                    </div>
                  ) : null}
                </div>
              </div>
            ) : (
              <div className="settings-field-group">
                <label htmlFor="ig-modal-webhook-url" className="settings-field-label">
                  <span>Webhook URL</span>
                </label>
                <input
                  id="ig-modal-webhook-url"
                  type="url"
                  value={webhookUrl}
                  onChange={(e) => setWebhookUrl(e.target.value)}
                  placeholder="https://hooks.zapier.com/hooks/catch/..."
                  className="settings-input"
                />
                <p className="settings-field-desc">
                  Clip metadata and output video path will be sent via JSON POST on publish.
                </p>
              </div>
            )}

            <div className="connection-test-row">
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
                  disabled={saving || localSavingCreds}
                >
                  {(saving || localSavingCreds) ? <Loader2 className="spin" size={13} /> : <Key size={13} />}
                  <span>{saving || localSavingCreds ? "Saving..." : "Save Credentials"}</span>
                </button>
              )}
            </div>

            {testStatus && (
              <div
                className={`connection-status-banner ${
                  testStatus.success ? "success" : "error"
                }`}
              >
                {testStatus.success ? <BadgeCheck size={16} /> : <AlertTriangle size={16} />}
                <span>{testStatus.message}</span>
              </div>
            )}
          </div>
        ) : (
          /* ================= Studio View ================= */
          <div className="ig-studio-grid">
            {/* Left Pane: Clip details & Caption Editor */}
            <div className="ig-studio-left-pane">
              {/* Clip Metadata Banner */}
              {candidate && (
                <div className="ig-clip-meta-banner">
                  <div className="clip-meta-info">
                    <span className="clip-rank-pill">Clip #{candidate.rank}</span>
                    <strong className="clip-hook-text">{candidate.hook}</strong>
                  </div>
                  <div className="clip-score-badge">
                    <Flame size={12} />
                    <span>{Math.round(candidate.score * 100)}% Viral Score</span>
                  </div>
                </div>
              )}

              {/* Caption Option Selector Tabs */}
              <div className="ig-caption-studio-section">
                <div className="ig-caption-section-header">
                  <label className="settings-field-label">
                    <span>AI Caption Angles</span>
                  </label>
                  <button
                    type="button"
                    className="action-pill-btn"
                    onClick={() => void handleRegenerateCaptions()}
                    disabled={isRegenerating || publishing}
                    title="Regenerate all 3 caption angles with AI"
                  >
                    <RefreshCw className={isRegenerating ? "spin" : ""} size={11} />
                    <span>{isRegenerating ? "Regenerating..." : "Regenerate AI"}</span>
                  </button>
                </div>

                <div className="caption-option-tabs">
                  {captionOptions.map((opt) => (
                    <button
                      key={opt.style}
                      type="button"
                      className={`caption-tab-card ${
                        selectedStyle === opt.style ? "active" : ""
                      }`}
                      onClick={() => handleSelectStyle(opt.style)}
                    >
                      <div className="tab-card-header">
                        <span className="tab-card-title">{opt.title}</span>
                        {selectedStyle === opt.style && (
                          <span className="tab-active-dot" />
                        )}
                      </div>
                      <p className="tab-card-hook">
                        {opt.hook ? `"${opt.hook.slice(0, 50)}..."` : "Content-grounded"}
                      </p>
                    </button>
                  ))}
                </div>

                {/* Caption Textarea Editor */}
                <div className="caption-editor-wrapper">
                  <div className="caption-editor-header">
                    <span>Caption Content</span>
                    <span
                      className={`char-count ${
                        activeCaption.length > 2100 ? "near-limit" : ""
                      }`}
                    >
                      {activeCaption.length} / 2,200 chars
                    </span>
                  </div>
                  <textarea
                    rows={6}
                    value={activeCaption}
                    onChange={(e) => handleCaptionChange(e.target.value)}
                    placeholder="Write an engaging caption for your Reel..."
                    className="caption-textarea"
                    spellCheck={false}
                  />
                </div>

                {/* Hashtag Chips Editor */}
                <div className="ig-hashtags-section">
                  <div className="caption-editor-header">
                    <span>Target Hashtags</span>
                    <span className="hashtag-count">{hashtags.length} tags</span>
                  </div>
                  <div className="hashtag-chips-container">
                    {hashtags.map((tag) => (
                      <span key={tag} className="ig-hashtag-chip">
                        <span>{tag}</span>
                        <button
                          type="button"
                          className="chip-remove-btn"
                          onClick={() => handleRemoveHashtag(tag)}
                          aria-label={`Remove ${tag}`}
                        >
                          <X size={10} />
                        </button>
                      </span>
                    ))}
                    <div className="hashtag-input-chip">
                      <input
                        type="text"
                        value={newTagInput}
                        onChange={(e) => setNewTagInput(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            e.preventDefault();
                            handleAddHashtag();
                          }
                        }}
                        placeholder="+ add tag"
                        className="hashtag-mini-input"
                      />
                      {newTagInput.trim() && (
                        <button
                          type="button"
                          className="chip-add-btn"
                          onClick={handleAddHashtag}
                        >
                          <Plus size={11} />
                        </button>
                      )}
                    </div>
                  </div>
                </div>
              </div>
            </div>

            {/* Right Pane: Live Instagram Reels Phone Mockup */}
            <div className="ig-studio-right-pane">
              <div className="reels-mockup-frame">
                <div className="phone-notch">
                  <span className="notch-speaker" />
                </div>

                {/* Video Playback / Fallback View */}
                <div className="phone-screen-content">
                  {videoSrc ? (
                    <video
                      ref={videoRef}
                      src={videoSrc}
                      className="reel-video-element"
                      loop
                      playsInline
                      onClick={() => {
                        if (videoRef.current) {
                          if (videoPlaying) {
                            videoRef.current.pause();
                            setVideoPlaying(false);
                          } else {
                            videoRef.current.play().catch(() => {});
                            setVideoPlaying(true);
                          }
                        }
                      }}
                    />
                  ) : (
                    <div className="reel-poster-fallback">
                      <div className="fallback-play-circle">
                        <Play size={24} />
                      </div>
                      <p>Clip will auto-render on publish</p>
                    </div>
                  )}

                  {/* Instagram Reels Overlay UI */}
                  <div className="reels-overlay-layer">
                    {/* Right Action Icons Column */}
                    <div className="reels-actions-col">
                      <div className="reels-action-item">
                        <div className="action-circle">
                          <Heart size={20} />
                        </div>
                        <span>12.4K</span>
                      </div>
                      <div className="reels-action-item">
                        <div className="action-circle">
                          <MessageCircle size={20} />
                        </div>
                        <span>348</span>
                      </div>
                      <div className="reels-action-item">
                        <div className="action-circle">
                          <Share2 size={20} />
                        </div>
                        <span>Share</span>
                      </div>
                      <div className="reels-action-item">
                        <div className="action-disc spin-slow">
                          <Music size={12} />
                        </div>
                      </div>
                    </div>

                    {/* Bottom Metadata & Dynamic Caption Overlay */}
                    <div className="reels-bottom-info">
                      <div className="reels-creator-row">
                        <div className="creator-avatar">
                          <span>CO</span>
                        </div>
                        <span className="creator-handle">@clipon_creator</span>
                        <button type="button" className="creator-follow-btn">
                          Follow
                        </button>
                      </div>

                      <div className="reels-caption-box">
                        <p className="reels-caption-text">
                          {activeCaption || "Your viral caption will preview here..."}
                        </p>
                        {hashtags.length > 0 && (
                          <p className="reels-tags-text">{hashtags.join(" ")}</p>
                        )}
                      </div>

                      <div className="reels-audio-tag">
                        <Music size={11} />
                        <span className="audio-marquee">Original Audio • ClipOn Reel Sync</span>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        )}

        {/* Live Publishing Multi-Step Progress Banner */}
        {publishing && (
          <div className="ig-publishing-stepper-box">
            <div className="stepper-header">
              <Loader2 className="spin" size={16} />
              <strong>Publishing Reel to Meta Servers...</strong>
            </div>
            <div className="stepper-stages">
              <div className={`step-item ${publishStep >= 1 ? "active" : ""}`}>
                <span className="step-num">1</span>
                <span>Video Validation</span>
              </div>
              <div className={`step-item ${publishStep >= 2 ? "active" : ""}`}>
                <span className="step-num">2</span>
                <span>Container Init</span>
              </div>
              <div className={`step-item ${publishStep >= 3 ? "active" : ""}`}>
                <span className="step-num">3</span>
                <span>Rupload Transfer</span>
              </div>
              <div className={`step-item ${publishStep >= 4 ? "active" : ""}`}>
                <span className="step-num">4</span>
                <span>Meta Transcoding</span>
              </div>
              <div className={`step-item ${publishStep >= 5 ? "active" : ""}`}>
                <span className="step-num">5</span>
                <span>Live!</span>
              </div>
            </div>
          </div>
        )}

        {/* Success Banner if Published */}
        {isPublished && postUrl && (
          <div className="ig-published-banner">
            <div className="published-banner-icon">
              <BadgeCheck size={24} />
            </div>
            <div className="published-banner-content">
              <strong>🎉 Reel is Published Live on Instagram!</strong>
              <p>Your video is transcoding and live in the Instagram Reels feed.</p>
              <div className="published-links-row">
                <button
                  type="button"
                  className="studio-btn primary small"
                  onClick={() => onOpenExternal(postUrl)}
                >
                  <ExternalLink size={12} />
                  <span>View Reel on Instagram</span>
                </button>
                <button
                  type="button"
                  className="studio-btn secondary small"
                  onClick={() => {
                    navigator.clipboard.writeText(postUrl);
                    onShowToast("Reel link copied!");
                  }}
                >
                  <Copy size={12} />
                  <span>Copy Reel URL</span>
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Failure Banner if Post Failed */}
        {isPublishFailed && existingPost?.errorMessage && (
          <div className="connection-status-banner error" style={{ margin: "16px 0 0 0" }}>
            <AlertTriangle size={18} style={{ flexShrink: 0 }} />
            <div>
              <strong>Instagram Publishing Failed</strong>
              <p style={{ margin: "4px 0 0 0", fontSize: "12px" }}>
                {existingPost.errorMessage}
              </p>
            </div>
          </div>
        )}
      </div>

      {/* Modal Footer Actions */}
      <div className="modal-footer ig-modal-footer">
        <div className="footer-left-status">
          <span
            className={`status-pill ${
              isAccountConfigured ? "active" : "missing"
            }`}
          >
            {isAccountConfigured
              ? provider === "graph_api"
                ? "Meta Graph API Linked"
                : "Webhook Linked"
              : "Not Configured"}
          </span>
          <button
            type="button"
            className="footer-copy-all-btn"
            onClick={handleCopyFullCaption}
            title="Copy caption and hashtags to clipboard"
          >
            <Copy size={13} />
            <span>{isCopied ? "Copied!" : "Copy Caption"}</span>
          </button>
        </div>

        <div className="footer-right-buttons">
          <button
            type="button"
            className="studio-btn secondary"
            onClick={onClose}
          >
            {isPublished ? "Done" : "Cancel"}
          </button>

          {activeTab === "studio" && (
            <button
              type="button"
              className="studio-btn instagram-active primary-publish-btn"
              onClick={() => void handlePublishToInstagram()}
              disabled={publishing || !isAccountConfigured}
            >
              {publishing ? (
                <Loader2 className="spin" size={15} />
              ) : (
                <Instagram size={15} />
              )}
              <span>
                {publishing
                  ? "Publishing to Reels..."
                  : isPublishFailed
                    ? "Retry Post to Reels"
                    : isPublished
                      ? "Re-post to Reels"
                      : "Post to Instagram Reels"}
              </span>
            </button>
          )}
        </div>
      </div>
    </AccessibleModal>
  );
}
