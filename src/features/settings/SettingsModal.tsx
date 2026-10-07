import { useState } from "react";
import {
  Settings,
  X,
  Cpu,
  HardDrive,
  Sliders,
  Activity,
  Download,
  Instagram,
  Loader2,
  BadgeCheck,
  AlertTriangle,
  FolderOpen,
  Trash2,
  ShieldAlert,
  Zap,
} from "lucide-react";
import { EnvironmentStatus, ReframeMode, SettingsTab } from "../../types";
import { AccessibleModal } from "../../components/AccessibleModal";

export interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  environment: EnvironmentStatus | null;
  transcriptionEngine: "deepgram" | "local";
  setTranscriptionEngine: (e: "deepgram" | "local") => void;
  deepgramKey: string;
  setDeepgramKey: (k: string) => void;
  llmEngine: string;
  setLlmEngine: (e: any) => void;
  localLlmModel: string;
  setLocalLlmModel: (m: string) => void;
  anthropicKey: string;
  setAnthropicKey: (k: string) => void;
  deepseekKey: string;
  setDeepseekKey: (k: string) => void;
  deepseekModel: string;
  setDeepseekModel: (m: string) => void;
  geminiKey: string;
  setGeminiKey: (k: string) => void;
  openaiKey: string;
  setOpenaiKey: (k: string) => void;
  groqKey: string;
  setGroqKey: (k: string) => void;
  nvidiaKey: string;
  setNvidiaKey: (k: string) => void;
  nvidiaFunctionId?: string;
  setNvidiaFunctionId?: (id: string) => void;
  instagramProvider: "graph_api" | "webhook";
  setInstagramProvider: (p: "graph_api" | "webhook") => void;
  instagramAccountId: string;
  setInstagramAccountId: (id: string) => void;
  instagramAccessToken: string;
  setInstagramAccessToken: (t: string) => void;
  instagramWebhookUrl: string;
  setInstagramWebhookUrl: (u: string) => void;
  testInstagramConnection: () => Promise<void>;
  instagramTesting: boolean;
  instagramTestResult: { success: boolean; message: string } | null;
  youtubeSaveDir: string;
  setYoutubeSaveDir: (d: string) => void;
  clipsSaveDir: string;
  setClipsSaveDir: (d: string) => void;
  defaultFolders: { youtubeSaveDir: string; clipsOutputDir: string } | null;
  reframeMode: ReframeMode;
  setReframeMode: (m: ReframeMode) => void;
  punchZoom: boolean;
  setPunchZoom: (z: boolean) => void;
  removeSilence: boolean;
  setRemoveSilence: (s: boolean) => void;
  studioAudio: boolean;
  setStudioAudio: (a: boolean) => void;
  pullModelDirectly: (m: string) => void;
  browseFolder: (
    initial: string,
    setter: (s: string) => void,
    storageKey: string
  ) => void;
  openFolder: (path: string) => void;
  onClearStorage: () => Promise<void>;
  onSaveAndClose: () => void;
}

export function SettingsModal({
  isOpen,
  onClose,
  environment,
  transcriptionEngine,
  setTranscriptionEngine,
  deepgramKey,
  setDeepgramKey,
  llmEngine,
  setLlmEngine,
  localLlmModel,
  setLocalLlmModel,
  anthropicKey,
  setAnthropicKey,
  deepseekKey,
  setDeepseekKey,
  deepseekModel,
  setDeepseekModel,
  geminiKey,
  setGeminiKey,
  openaiKey,
  setOpenaiKey,
  groqKey,
  setGroqKey,
  nvidiaKey,
  setNvidiaKey,
  nvidiaFunctionId = "",
  setNvidiaFunctionId,
  instagramProvider,
  setInstagramProvider,
  instagramAccountId,
  setInstagramAccountId,
  instagramAccessToken,
  setInstagramAccessToken,
  instagramWebhookUrl,
  setInstagramWebhookUrl,
  testInstagramConnection,
  instagramTesting,
  instagramTestResult,
  youtubeSaveDir,
  setYoutubeSaveDir,
  clipsSaveDir,
  setClipsSaveDir,
  defaultFolders,
  reframeMode,
  setReframeMode,
  punchZoom,
  setPunchZoom,
  removeSilence,
  setRemoveSilence,
  studioAudio,
  setStudioAudio,
  pullModelDirectly,
  browseFolder,
  openFolder,
  onClearStorage,
  onSaveAndClose,
}: SettingsModalProps) {
  const [settingsTab, setSettingsTab] = useState<SettingsTab>("ai");
  const [tosAck, setTosAck] = useState(
    () => localStorage.getItem("clipon_youtube_tos_ack") === "true"
  );

  if (!isOpen) return null;

  const handleTosToggle = (val: boolean) => {
    setTosAck(val);
    localStorage.setItem("clipon_youtube_tos_ack", val ? "true" : "false");
  };

  return (
    <AccessibleModal
      isOpen={isOpen}
      onClose={onClose}
      title="Studio Configuration"
      titleId="settings-modal-title"
      dialogClassName="settings-modal"
    >
      <div className="modal-header">
        <div className="modal-header-left">
          <div className="modal-icon-badge">
            <Settings size={18} />
          </div>
          <div>
            <h3 id="settings-modal-title">Studio Configuration</h3>
            <p>
              Tune AI models, storage destinations, rendering parameters &amp;
              accounts
            </p>
          </div>
        </div>
        <button
          className="modal-close-btn"
          onClick={onClose}
          aria-label="Close dialog"
        >
          <X size={16} />
        </button>
      </div>

      <div className="settings-tab-nav">
        <button
          className={`settings-nav-btn ${settingsTab === "ai" ? "active" : ""}`}
          onClick={() => setSettingsTab("ai")}
        >
          <Cpu size={14} /> AI Engines
        </button>
        <button
          className={`settings-nav-btn ${settingsTab === "storage" ? "active" : ""}`}
          onClick={() => setSettingsTab("storage")}
        >
          <HardDrive size={14} /> Storage &amp; Folders
        </button>
        <button
          className={`settings-nav-btn ${settingsTab === "export" ? "active" : ""}`}
          onClick={() => setSettingsTab("export")}
        >
          <Sliders size={14} /> Export &amp; Video
        </button>
        <button
          className={`settings-nav-btn ${settingsTab === "system" ? "active" : ""}`}
          onClick={() => setSettingsTab("system")}
        >
          <Activity size={14} /> System &amp; Diagnostics
        </button>
      </div>

      <div className="settings-tab-content">
        {settingsTab === "ai" && (
          <div className="settings-form-stack">
            <div className="settings-field-group">
              <label>Transcription Provider</label>
              <select
                value={transcriptionEngine}
                onChange={(e) => setTranscriptionEngine(e.target.value as any)}
              >
                <option value="local">
                  Local Whisper (Offline &amp; Free)
                </option>
                <option value="deepgram">
                  Deepgram (Cloud API - Super Fast)
                </option>
              </select>
            </div>

            {transcriptionEngine === "deepgram" && (
              <div className="settings-field-group">
                <label>Deepgram API Key</label>
                <input
                  type="password"
                  value={deepgramKey}
                  onChange={(e) => setDeepgramKey(e.target.value)}
                  placeholder={
                    environment?.hasDeepgramKey
                      ? "Loaded securely"
                      : "Enter Deepgram API Key"
                  }
                />
              </div>
            )}

            <div className="settings-field-group">
              <label>Viral Moment LLM Provider</label>
              <select
                value={llmEngine}
                onChange={(e) => setLlmEngine(e.target.value as any)}
              >
                <option value="local">Ollama (Offline Local)</option>
                <option value="claude">Anthropic Claude</option>
                <option value="deepseek">DeepSeek AI</option>
                <option value="gemini">Google Gemini</option>
                <option value="openai">OpenAI</option>
                <option value="openrouter">OpenRouter</option>
                <option value="groq">Groq (Ultra-Fast)</option>
              </select>
            </div>

            {llmEngine === "local" && (
              <div className="settings-field-group">
                <label>Ollama Model</label>
                <div className="input-with-button">
                  <input
                    type="text"
                    value={localLlmModel}
                    onChange={(e) => setLocalLlmModel(e.target.value)}
                    placeholder="e.g. llama3.2, qwen2.5:7b"
                  />
                  <button
                    type="button"
                    className="studio-btn secondary"
                    onClick={() => pullModelDirectly(localLlmModel)}
                  >
                    <Download size={13} />
                    Pull Model
                  </button>
                </div>
              </div>
            )}

            {llmEngine === "claude" && (
              <div className="settings-field-group">
                <label>Anthropic API Key</label>
                <input
                  type="password"
                  value={anthropicKey}
                  onChange={(e) => setAnthropicKey(e.target.value)}
                  placeholder={
                    environment?.hasAnthropicKey
                      ? "Loaded securely"
                      : "Enter Anthropic API Key"
                  }
                />
              </div>
            )}

            {llmEngine === "deepseek" && (
              <>
                <div className="settings-field-group">
                  <label>DeepSeek API Key</label>
                  <input
                    type="password"
                    value={deepseekKey}
                    onChange={(e) => setDeepseekKey(e.target.value)}
                    placeholder={
                      environment?.hasDeepseekKey
                        ? "Loaded securely"
                        : "Enter DeepSeek API Key"
                    }
                  />
                </div>
                <div className="settings-field-group">
                  <label>DeepSeek Model Name (Optional)</label>
                  <input
                    type="text"
                    value={deepseekModel}
                    onChange={(e) => setDeepseekModel(e.target.value)}
                    placeholder="e.g. deepseek-chat"
                  />
                </div>
              </>
            )}

            {llmEngine === "gemini" && (
              <div className="settings-field-group">
                <label>Google Gemini API Key</label>
                <input
                  type="password"
                  value={geminiKey}
                  onChange={(e) => setGeminiKey(e.target.value)}
                  placeholder={
                    environment?.hasGeminiKey
                      ? "Loaded securely"
                      : "Enter Gemini API Key"
                  }
                />
              </div>
            )}

            {llmEngine === "openai" && (
              <div className="settings-field-group">
                <label>OpenAI API Key</label>
                <input
                  type="password"
                  value={openaiKey}
                  onChange={(e) => setOpenaiKey(e.target.value)}
                  placeholder={
                    environment?.hasOpenaiKey
                      ? "Loaded securely"
                      : "Enter OpenAI API Key"
                  }
                />
              </div>
            )}

            {llmEngine === "groq" && (
              <div className="settings-field-group">
                <label>Groq API Key</label>
                <input
                  type="password"
                  value={groqKey}
                  onChange={(e) => setGroqKey(e.target.value)}
                  placeholder={
                    environment?.hasGroqKey
                      ? "Loaded securely"
                      : "Enter Groq API Key"
                  }
                />
              </div>
            )}

            {/* Active Speaker Detection API */}
            <div className="settings-section-divider">
              <Zap size={14} />
              <span>NVIDIA Active Speaker Detection API</span>
            </div>

            <div className="settings-field-group">
              <label>NVIDIA API Key</label>
              <input
                type="password"
                value={nvidiaKey}
                onChange={(e) => setNvidiaKey(e.target.value)}
                placeholder={
                  environment?.hasNvidiaKey
                    ? "Loaded securely in Keychain"
                    : "Enter NVIDIA API Key (nvapi-...)"
                }
              />
              <span className="folder-hint">
                Primary engine for Active Speaker Detection &amp; speaker-person fusion. Stored securely in OS Keyring.
              </span>
            </div>

            <div className="settings-field-group">
              <label>NVIDIA ASD Function ID (Optional Preview UUID)</label>
              <input
                type="text"
                value={nvidiaFunctionId}
                onChange={(e) => setNvidiaFunctionId?.(e.target.value)}
                placeholder={
                  environment?.hasNvidiaFunctionId
                    ? "Configured in Keychain / Env"
                    : "e.g. 12345678-abcd-ef01-2345-6789abcdef01"
                }
              />
              <span className="folder-hint">
                NVCF Function ID for NVIDIA Active Speaker Detection preview endpoint. Defaults to multimodal local fusion if omitted.
              </span>
            </div>

            {/* Active Speaker Provider Status Card */}
            <div
              className={`settings-provider-card ${
                environment?.activeSpeakerProvider === "NVIDIA"
                  ? "nvidia-active"
                  : "fallback-active"
              }`}
            >
              <div className="settings-provider-header">
                <div className="settings-provider-title">
                  <Zap
                    size={14}
                    className={
                      environment?.activeSpeakerProvider === "NVIDIA"
                        ? "provider-icon-green"
                        : "provider-icon-amber"
                    }
                  />
                  <span>Active Speaker Provider:</span>
                  <strong>
                    {environment?.activeSpeakerProvider || "Local fallback"}
                  </strong>
                </div>
                <span
                  className={`status-pill ${
                    environment?.activeSpeakerProvider === "NVIDIA"
                      ? "pill-green"
                      : "pill-amber"
                  }`}
                >
                  {environment?.activeSpeakerProvider === "NVIDIA"
                    ? "⚡ NVIDIA NIM Active"
                    : "⚠️ Local Fallback Active"}
                </span>
              </div>
              <p className="settings-provider-desc">
                {environment?.activeSpeakerStatus ||
                  (environment?.hasNvidiaKey
                    ? "NVIDIA NIM ASD is configured as the active speaker detection engine."
                    : "NVIDIA API key not set. ClipOn uses Local Fallback (Apple Vision face tracking + diarization temporal fusion). Provide an NVIDIA API Key and NVCF Function ID above to enable neural active speaker inference.")}
              </p>
            </div>

            {/* Instagram Reels API */}
            <div className="settings-section-divider">
              <Instagram size={14} />
              <span>Instagram Reels API</span>
            </div>

            <div className="settings-field-group">
              <label>Instagram Publishing Method</label>
              <select
                value={instagramProvider}
                onChange={(e) => setInstagramProvider(e.target.value as any)}
              >
                <option value="graph_api">
                  Official Meta Graph API (Direct Instagram Reels)
                </option>
                <option value="webhook">
                  Webhook Automation (Make.com, Zapier, n8n)
                </option>
              </select>
            </div>

            {instagramProvider === "graph_api" && (
              <>
                <div className="settings-field-group">
                  <label>Instagram Professional / Creator Account ID</label>
                  <input
                    type="text"
                    value={instagramAccountId}
                    onChange={(e) => setInstagramAccountId(e.target.value)}
                    placeholder="e.g. 17841400000000000"
                  />
                  <span className="folder-hint">
                    Found in Meta Business Suite or via Graph API Explorer
                  </span>
                </div>

                <div className="settings-field-group">
                  <label>Meta Long-Lived Access Token</label>
                  <input
                    type="password"
                    value={instagramAccessToken}
                    onChange={(e) => setInstagramAccessToken(e.target.value)}
                    placeholder={
                      environment?.hasInstagramToken
                        ? "Loaded securely in Keychain"
                        : "EAA... (Token with instagram_content_publish permission)"
                    }
                  />
                  <span className="folder-hint">
                    Stored securely in OS Keychain. Requires 'instagram_basic'
                    and 'instagram_content_publish' scopes
                  </span>
                </div>
              </>
            )}

            {instagramProvider === "webhook" && (
              <div className="settings-field-group">
                <label>Webhook URL (Make.com / Zapier / n8n)</label>
                <input
                  type="text"
                  value={instagramWebhookUrl}
                  onChange={(e) => setInstagramWebhookUrl(e.target.value)}
                  placeholder="https://hook.eu1.make.com/... or https://hooks.zapier.com/..."
                />
                <span className="folder-hint">
                  Payload includes candidateId, videoPath, viralScore, hook, and
                  formatted caption
                </span>
              </div>
            )}

            <div
              style={{
                marginTop: "4px",
                display: "flex",
                alignItems: "center",
                gap: "12px",
              }}
            >
              <button
                type="button"
                className="studio-btn secondary"
                onClick={testInstagramConnection}
                disabled={
                  instagramTesting ||
                  (instagramProvider === "graph_api"
                    ? !instagramAccountId.trim() ||
                      (!instagramAccessToken.trim() &&
                        !environment?.hasInstagramToken)
                    : !instagramWebhookUrl.trim())
                }
              >
                {instagramTesting ? (
                  <Loader2 className="spin" size={14} />
                ) : (
                  <Instagram size={14} />
                )}
                <span>
                  {instagramTesting ? "Verifying..." : "Test Connection"}
                </span>
              </button>
            </div>

            {instagramTestResult && (
              <div
                className={`connection-status-banner ${instagramTestResult.success ? "success" : "error"}`}
              >
                {instagramTestResult.success ? (
                  <BadgeCheck size={16} />
                ) : (
                  <AlertTriangle size={16} />
                )}
                <span>{instagramTestResult.message}</span>
              </div>
            )}
          </div>
        )}

        {settingsTab === "storage" && (
          <div className="settings-form-stack">
            {/* YouTube Downloads Folder */}
            <div className="settings-folder-group">
              <div className="folder-group-header">
                <label>📥 YouTube Downloads Destination</label>
                <span className="folder-hint">
                  Where downloaded YouTube videos will be stored
                </span>
              </div>
              <div className="folder-input-row">
                <input
                  type="text"
                  value={youtubeSaveDir}
                  onChange={(e) => setYoutubeSaveDir(e.target.value)}
                  placeholder={
                    defaultFolders?.youtubeSaveDir || "~/Downloads/ClipOn"
                  }
                />
                <button
                  className="studio-btn secondary"
                  onClick={() =>
                    browseFolder(
                      youtubeSaveDir || defaultFolders?.youtubeSaveDir || "",
                      setYoutubeSaveDir,
                      "clipon_youtube_dir"
                    )
                  }
                  title="Pick folder visually"
                >
                  Browse...
                </button>
                <button
                  className="studio-btn secondary icon-only"
                  onClick={() =>
                    openFolder(
                      youtubeSaveDir || defaultFolders?.youtubeSaveDir || ""
                    )
                  }
                  title="Open folder in Finder"
                >
                  <FolderOpen size={15} />
                </button>
              </div>
            </div>

            {/* YouTube Terms of Service Compliance Policy */}
            <div
              className="settings-folder-group"
              style={{
                borderColor: "rgba(59, 130, 246, 0.3)",
                background: "rgba(59, 130, 246, 0.04)",
              }}
            >
              <div className="folder-group-header">
                <label
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                    color: "#93c5fd",
                  }}
                >
                  <ShieldAlert size={14} color="#60a5fa" />
                  YouTube Terms &amp; Compliance Policy
                </label>
                <span className="folder-hint">
                  Required acknowledgment before utilizing the automated YouTube
                  media downloader
                </span>
              </div>
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  marginTop: 6,
                }}
              >
                <input
                  type="checkbox"
                  id="tos_settings_ack"
                  checked={tosAck}
                  onChange={(e) => handleTosToggle(e.target.checked)}
                  style={{ cursor: "pointer" }}
                />
                <label
                  htmlFor="tos_settings_ack"
                  style={{
                    fontSize: 12,
                    color: "#e2e8f0",
                    cursor: "pointer",
                    userSelect: "none",
                  }}
                >
                  I acknowledge YouTube Terms of Service and accept
                  responsibility for copyright verification.
                </label>
              </div>
            </div>

            {/* Clips Output Folder */}
            <div className="settings-folder-group">
              <div className="folder-group-header">
                <label>Rendered Clips Output Destination</label>
                <span className="folder-hint">
                  Where final vertical video clips and captions will be saved
                </span>
              </div>
              <div className="folder-input-row">
                <input
                  type="text"
                  value={clipsSaveDir}
                  onChange={(e) => setClipsSaveDir(e.target.value)}
                  placeholder={
                    defaultFolders?.clipsOutputDir || "~/Documents/ClipOn"
                  }
                />
                <button
                  className="studio-btn secondary"
                  onClick={() =>
                    browseFolder(
                      clipsSaveDir || defaultFolders?.clipsOutputDir || "",
                      setClipsSaveDir,
                      "clipon_clips_dir"
                    )
                  }
                  title="Pick folder visually"
                >
                  Browse...
                </button>
                <button
                  className="studio-btn secondary icon-only"
                  onClick={() =>
                    openFolder(
                      clipsSaveDir || defaultFolders?.clipsOutputDir || ""
                    )
                  }
                  title="Open folder in Finder"
                >
                  <FolderOpen size={15} />
                </button>
              </div>
            </div>

            {/* Storage Cleanup */}
            <div
              className="settings-folder-group"
              style={{
                borderColor: "rgba(239, 68, 68, 0.25)",
                background: "rgba(239, 68, 68, 0.03)",
              }}
            >
              <div className="folder-group-header">
                <label style={{ color: "#f87171" }}>
                  🗑️ Project Storage Cleanup
                </label>
                <span className="folder-hint">
                  Delete all rendered vertical clips, downloaded source videos,
                  and clear database history
                </span>
              </div>
              <div style={{ marginTop: "4px" }}>
                <button
                  type="button"
                  className="studio-btn secondary"
                  style={{
                    color: "#f87171",
                    borderColor: "rgba(239, 68, 68, 0.4)",
                  }}
                  onClick={onClearStorage}
                >
                  <Trash2 size={14} /> Clear Full Storage
                </button>
              </div>
            </div>
          </div>
        )}

        {settingsTab === "export" && (
          <div className="settings-form-stack">
            <div className="settings-field-group">
              <label>Default Video Framing</label>
              <select
                value={reframeMode}
                onChange={(e) => setReframeMode(e.target.value as ReframeMode)}
              >
                <option value="vertical_crop">
                  1. Center Crop (Standard 9:16)
                </option>
                <option
                  value="smart_face_track"
                  disabled={environment?.faceTrackingSupported === false}
                >
                  2. Smart Face Tracking (AI Center 9:16)
                  {environment?.faceTrackingSupported === false
                    ? " — macOS only"
                    : ""}
                </option>
                <option value="original">3. Original Aspect Ratio</option>
              </select>
            </div>

            <div className="settings-field-group">
              <label>Retention Punch Zoom</label>
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 10,
                  marginTop: 4,
                }}
              >
                <input
                  type="checkbox"
                  id="setting_punch_zoom"
                  checked={punchZoom}
                  onChange={(e) => {
                    setPunchZoom(e.target.checked);
                    localStorage.setItem(
                      "clipon_punch_zoom",
                      String(e.target.checked)
                    );
                  }}
                  style={{
                    width: 16,
                    height: 16,
                    accentColor: "#a855f7",
                    cursor: "pointer",
                  }}
                />
                <label
                  htmlFor="setting_punch_zoom"
                  style={{
                    margin: 0,
                    cursor: "pointer",
                    fontSize: 13,
                    color: "var(--text-secondary)",
                  }}
                >
                  Retention Punch Zoom Cuts (Punches 1.14x visual zoom every
                  5.5s to maintain viewer attention)
                </label>
              </div>
            </div>

            <div className="settings-field-group">
              <label>Dead-Air Silence Jump Cutter</label>
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 10,
                  marginTop: 4,
                }}
              >
                <input
                  type="checkbox"
                  id="setting_remove_silence"
                  checked={removeSilence}
                  onChange={(e) => {
                    setRemoveSilence(e.target.checked);
                    localStorage.setItem(
                      "clipon_remove_silence",
                      String(e.target.checked)
                    );
                  }}
                  style={{
                    width: 16,
                    height: 16,
                    accentColor: "#10b981",
                    cursor: "pointer",
                  }}
                />
                <label
                  htmlFor="setting_remove_silence"
                  style={{
                    margin: 0,
                    cursor: "pointer",
                    fontSize: 13,
                    color: "var(--text-secondary)",
                  }}
                >
                  Automatically skip pauses &amp; dead air &gt;0.45s (Boosts
                  video retention by 20%)
                </label>
              </div>
            </div>

            <div className="settings-field-group">
              <label>Studio Sound Mastering</label>
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 10,
                  marginTop: 4,
                }}
              >
                <input
                  type="checkbox"
                  id="setting_studio_audio"
                  checked={studioAudio}
                  onChange={(e) => {
                    setStudioAudio(e.target.checked);
                    localStorage.setItem(
                      "clipon_studio_audio",
                      String(e.target.checked)
                    );
                  }}
                  style={{
                    width: 16,
                    height: 16,
                    accentColor: "#facc15",
                    cursor: "pointer",
                  }}
                />
                <label
                  htmlFor="setting_studio_audio"
                  style={{
                    margin: 0,
                    cursor: "pointer",
                    fontSize: 13,
                    color: "var(--text-secondary)",
                  }}
                >
                  Auto-Master Audio to -14 LUFS Broadcast Standard with AI
                  Spectral Noise Suppression
                </label>
              </div>
            </div>
          </div>
        )}

        {settingsTab === "system" && (
          <div className="settings-form-stack">
            <div className="diagnostics-list">
              <div className="diag-item">
                <span className="diag-name">
                  Apple Silicon VideoToolbox Hardware Accel
                </span>
                <span
                  className={`diag-badge ${environment?.hasHardwareAccel ? "ok" : "muted"}`}
                >
                  {environment?.hasHardwareAccel
                    ? "Active (Hardware Accelerated)"
                    : "Inactive (CPU)"}
                </span>
              </div>
              <div className="diag-item">
                <span className="diag-name">FFmpeg</span>
                <span
                  className={`diag-badge ${environment?.hasFfmpeg ? "ok" : "err"}`}
                >
                  {environment?.hasFfmpeg ? "Installed" : "Missing"}
                </span>
              </div>
              <div className="diag-item">
                <span className="diag-name">FFprobe</span>
                <span
                  className={`diag-badge ${environment?.hasFfprobe ? "ok" : "err"}`}
                >
                  {environment?.hasFfprobe ? "Installed" : "Missing"}
                </span>
              </div>
              <div className="diag-item">
                <span className="diag-name">yt-dlp (YouTube downloader)</span>
                <span
                  className={`diag-badge ${environment?.hasYtdlp ? "ok" : "err"}`}
                >
                  {environment?.hasYtdlp ? "Installed" : "Missing"}
                </span>
              </div>
              <div className="diag-item">
                <span className="diag-name">
                  Local Whisper (openai-whisper)
                </span>
                <span
                  className={`diag-badge ${environment?.hasLocalWhisperModel ? "ok" : "err"}`}
                >
                  {environment?.hasLocalWhisperModel ? "Installed" : "Missing"}
                </span>
              </div>
              <div className="diag-item">
                <span className="diag-name">Ollama Local Daemon</span>
                <span
                  className={`diag-badge ${environment?.hasOllama ? "ok" : "err"}`}
                >
                  {environment?.hasOllama
                    ? "Running (127.0.0.1:11434)"
                    : "Not detected"}
                </span>
              </div>
              <div className="diag-item">
                <span className="diag-name">
                  Active Speaker Detection Provider
                </span>
                <span
                  className={`diag-badge ${
                    environment?.activeSpeakerProvider === "NVIDIA"
                      ? "ok"
                      : "warn"
                  }`}
                >
                  {environment?.activeSpeakerProvider === "NVIDIA"
                    ? "NVIDIA NIM ASD (Neural)"
                    : "Local Fallback (Apple Vision)"}
                </span>
              </div>
            </div>

            <div className="danger-zone">
              <label>Danger Zone</label>
              <p>
                Reset all configuration and restart onboarding from scratch.
              </p>
              <button
                className="studio-btn danger"
                onClick={() => {
                  if (
                    window.confirm("Reset all settings and restart onboarding?")
                  ) {
                    localStorage.clear();
                    window.location.reload();
                  }
                }}
              >
                Reset Configuration &amp; Onboarding
              </button>
            </div>
          </div>
        )}
      </div>

      <div className="modal-footer">
        <button className="studio-btn primary" onClick={onSaveAndClose}>
          Done
        </button>
      </div>
    </AccessibleModal>
  );
}
