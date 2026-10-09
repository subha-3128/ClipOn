import React, { useState, useMemo } from "react";
import {
  Settings,
  X,
  AudioLines,
  Loader2,
  CheckCircle2,
  AlertTriangle,
  Folder,
  ExternalLink,
  RotateCcw,
  Download,
  Trash2,
  Zap,
  Instagram,
  Youtube,
} from "lucide-react";
import {
  EnvironmentStatus,
  ReframeMode,
  SettingsTab,
  LlmEngine,
} from "../../types";
import { AccessibleModal } from "../../components/AccessibleModal";
import { SettingsSidebar } from "./SettingsSidebar";
import { SettingsFooter } from "./SettingsFooter";
import { SettingsSecretField } from "./SettingsSecretField";

export interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  environment: EnvironmentStatus | null;
  transcriptionEngine: "deepgram" | "local";
  setTranscriptionEngine: (e: "deepgram" | "local") => void;
  deepgramKey: string;
  setDeepgramKey: (k: string) => void;
  llmEngine: string;
  setLlmEngine: (e: LlmEngine) => void;
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
  youtubeClientId: string;
  setYoutubeClientId: (id: string) => void;
  youtubeClientSecret: string;
  setYoutubeClientSecret: (s: string) => void;
  youtubeRefreshToken: string;
  setYoutubeRefreshToken: (t: string) => void;
  testYoutubeConnection: () => Promise<void>;
  youtubeTesting: boolean;
  youtubeTestResult: { success: boolean; message: string } | null;
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
  onDeleteCredential: (name: string) => Promise<void>;
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
  youtubeClientId,
  setYoutubeClientId,
  youtubeClientSecret,
  setYoutubeClientSecret,
  youtubeRefreshToken,
  setYoutubeRefreshToken,
  testYoutubeConnection,
  youtubeTesting,
  youtubeTestResult,
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
  onDeleteCredential,
  onSaveAndClose,
}: SettingsModalProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("ai");
  const [credentialToDelete, setCredentialToDelete] = useState("gemini");
  const [pullingModel, setPullingModel] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [tosAck, setTosAck] = useState(
    () => localStorage.getItem("clipon_youtube_tos_ack") === "true"
  );

  const handleTosToggle = (val: boolean) => {
    setTosAck(val);
    localStorage.setItem("clipon_youtube_tos_ack", val ? "true" : "false");
  };

  const handlePullModel = async (model: string) => {
    setPullingModel(true);
    try {
      await pullModelDirectly(model);
    } finally {
      setPullingModel(false);
    }
  };

  const handleSave = async () => {
    setIsSaving(true);
    try {
      await onSaveAndClose();
    } finally {
      setIsSaving(false);
    }
  };

  const hasUnsavedChanges = useMemo(() => {
    return Boolean(
      deepgramKey.trim() ||
      geminiKey.trim() ||
      openaiKey.trim() ||
      anthropicKey.trim() ||
      deepseekKey.trim() ||
      groqKey.trim() ||
      nvidiaKey.trim() ||
      (nvidiaFunctionId && nvidiaFunctionId.trim()) ||
      instagramAccessToken.trim() ||
      youtubeClientSecret.trim() ||
      youtubeRefreshToken.trim()
    );
  }, [
    deepgramKey,
    geminiKey,
    openaiKey,
    anthropicKey,
    deepseekKey,
    groqKey,
    nvidiaKey,
    nvidiaFunctionId,
    instagramAccessToken,
    youtubeClientSecret,
    youtubeRefreshToken,
  ]);

  if (!isOpen) return null;

  return (
    <AccessibleModal
      isOpen={isOpen}
      onClose={onClose}
      title="Studio Settings"
      titleId="settings-modal-title"
      dialogClassName="settings-modal"
    >
      {/* Modal Top Header */}
      <div className="modal-header settings-header">
        <div className="modal-header-left">
          <div className="modal-icon-badge settings">
            <Settings size={18} />
          </div>
          <div>
            <h3 id="settings-modal-title">Studio Settings</h3>
            <p>
              Tune AI intelligence, speech transcription, 9:16 vertical reframe,
              storage paths, and accounts.
            </p>
          </div>
        </div>
        <button
          className="modal-close-btn"
          onClick={onClose}
          aria-label="Close studio settings dialog"
        >
          <X size={16} />
        </button>
      </div>

      {/* Two-Level Settings Layout */}
      <div className="settings-layout">
        {/* Left Navigation Sidebar */}
        <SettingsSidebar
          activeTab={activeTab}
          onSelectTab={setActiveTab}
          environment={environment}
        />

        {/* Right Active Content Pane */}
        <main className="settings-content-pane" role="tabpanel">
          {/* =================================================================
             1. AI & Intelligence Tab
             ================================================================= */}
          {activeTab === "ai" && (
            <div className="settings-tab-pane">
              <div className="pane-header">
                <div>
                  <h4>AI & Hook Detection</h4>
                  <p>
                    Select which LLM model scans transcripts for high-retention
                    hooks and viral moments.
                  </p>
                </div>
              </div>

              {/* Active Engine Card */}
              <div className="settings-card highlight">
                <div className="settings-field-group">
                  <label htmlFor="llm-engine-select" className="settings-field-label">
                    <span>Active Detection Model</span>
                    <span className="status-pill active">
                      <Zap size={11} /> Primary Analyzer
                    </span>
                  </label>
                  <p className="settings-field-desc">
                    ClipOn sends dialogue segments to this provider to detect
                    punchy soundbites and evaluate audience retention.
                  </p>
                  <select
                    id="llm-engine-select"
                    value={llmEngine}
                    onChange={(e) => {
                      const val = e.target.value as LlmEngine;
                      setLlmEngine(val);
                      localStorage.setItem("clipon_llm_engine", val);
                    }}
                    className="settings-select"
                  >
                    <option value="gemini">Google Gemini 2.5 (Fast &amp; Accurate - Recommended)</option>
                    <option value="openai">OpenAI GPT-4o / GPT-4o-mini</option>
                    <option value="anthropic">Anthropic Claude 3.5 Sonnet</option>
                    <option value="deepseek">DeepSeek Chat / Reasoner</option>
                    <option value="groq">Groq Cloud (Llama 3 70B - Ultra Fast)</option>
                    <option value="ollama">Ollama Local (100% Private On-Device)</option>
                    <option value="openrouter">OpenRouter Unified API</option>
                  </select>
                </div>
              </div>

              {/* Provider API Keys & Credentials */}
              <div className="settings-section-divider">
                <span>API Keys &amp; Models</span>
              </div>

              {/* Gemini */}
              <div className={`settings-card ${llmEngine === "gemini" ? "selected" : ""}`}>
                <SettingsSecretField
                  id="gemini-key"
                  label="Google Gemini API Key"
                  description="Used by default for fast, multimodal hook detection (gemini-2.5-flash / gemini-2.5-pro)."
                  value={geminiKey}
                  onChange={setGeminiKey}
                  isSavedInKeystore={Boolean(environment?.hasGeminiKey)}
                  onDeleteCredential={() => onDeleteCredential("gemini")}
                  placeholder="AIzaSy..."
                />
              </div>

              {/* OpenAI */}
              <div className={`settings-card ${llmEngine === "openai" ? "selected" : ""}`}>
                <SettingsSecretField
                  id="openai-key"
                  label="OpenAI API Key"
                  description="Used for GPT-4o and GPT-4o-mini viral segment scoring."
                  value={openaiKey}
                  onChange={setOpenaiKey}
                  isSavedInKeystore={Boolean(environment?.hasOpenaiKey)}
                  onDeleteCredential={() => onDeleteCredential("openai")}
                  placeholder="sk-..."
                />
              </div>

              {/* Anthropic Claude */}
              <div className={`settings-card ${llmEngine === "anthropic" ? "selected" : ""}`}>
                <SettingsSecretField
                  id="anthropic-key"
                  label="Anthropic Claude API Key"
                  description="Used for high-retention dialogue analysis with Claude 3.5 Sonnet."
                  value={anthropicKey}
                  onChange={setAnthropicKey}
                  isSavedInKeystore={Boolean(environment?.hasAnthropicKey)}
                  onDeleteCredential={() => onDeleteCredential("anthropic")}
                  placeholder="sk-ant-..."
                />
              </div>

              {/* DeepSeek */}
              <div className={`settings-card ${llmEngine === "deepseek" ? "selected" : ""}`}>
                <SettingsSecretField
                  id="deepseek-key"
                  label="DeepSeek API Key"
                  description="Used for DeepSeek reasoning and candidate hook ranking."
                  value={deepseekKey}
                  onChange={setDeepseekKey}
                  isSavedInKeystore={Boolean(environment?.hasDeepseekKey)}
                  onDeleteCredential={() => onDeleteCredential("deepseek")}
                  placeholder="sk-..."
                />
                <div className="settings-field-group sub-field">
                  <label htmlFor="deepseek-model" className="settings-field-label">
                    <span>DeepSeek Model Identifier</span>
                  </label>
                  <input
                    id="deepseek-model"
                    type="text"
                    value={deepseekModel}
                    onChange={(e) => {
                      setDeepseekModel(e.target.value);
                      localStorage.setItem("clipon_deepseek_model", e.target.value);
                    }}
                    placeholder="deepseek-chat (default)"
                    className="settings-input"
                  />
                </div>
              </div>

              {/* Groq Cloud */}
              <div className={`settings-card ${llmEngine === "groq" ? "selected" : ""}`}>
                <SettingsSecretField
                  id="groq-key"
                  label="Groq Cloud API Key"
                  description="Sub-second inference speed running Llama 3 70B."
                  value={groqKey}
                  onChange={setGroqKey}
                  isSavedInKeystore={Boolean(environment?.hasGroqKey)}
                  onDeleteCredential={() => onDeleteCredential("groq")}
                  placeholder="gsk_..."
                />
              </div>

              {/* Ollama Local */}
              <div className={`settings-card ${llmEngine === "ollama" ? "selected" : ""}`}>
                <div className="settings-field-group">
                  <div className="settings-field-header">
                    <label htmlFor="ollama-model" className="settings-field-label">
                      <span>Ollama Local Model (On-Device)</span>
                    </label>
                    <span className={`status-pill ${environment?.hasOllama ? "active" : "missing"}`}>
                      {environment?.hasOllama ? "Ollama Connected" : "Ollama Not Detected"}
                    </span>
                  </div>
                  <p className="settings-field-desc">
                    Runs 100% locally on your machine with zero cloud dependencies or API keys.
                  </p>
                  <div className="input-with-action-btn">
                    <input
                      id="ollama-model"
                      type="text"
                      value={localLlmModel}
                      onChange={(e) => {
                        setLocalLlmModel(e.target.value);
                        localStorage.setItem("clipon_local_llm_model", e.target.value);
                      }}
                      placeholder="llama3.2"
                      className="settings-input"
                    />
                    <button
                      type="button"
                      className="studio-btn secondary small"
                      onClick={() => handlePullModel(localLlmModel.trim() || "llama3.2")}
                      disabled={pullingModel || !environment?.hasOllama}
                      title="Pull model weights using local Ollama daemon"
                    >
                      {pullingModel ? (
                        <Loader2 className="spin" size={13} />
                      ) : (
                        <Download size={13} />
                      )}
                      <span>{pullingModel ? "Pulling..." : "Pull Model"}</span>
                    </button>
                  </div>
                  <div className="model-preset-pills">
                    <span className="preset-label">Recommended:</span>
                    {["llama3.2", "mistral", "gemma2", "qwen2.5"].map((m) => (
                      <button
                        key={m}
                        type="button"
                        className={`preset-pill ${localLlmModel === m ? "active" : ""}`}
                        onClick={() => {
                          setLocalLlmModel(m);
                          localStorage.setItem("clipon_local_llm_model", m);
                        }}
                      >
                        {m}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* =================================================================
             2. Transcription Tab
             ================================================================= */}
          {activeTab === "transcription" && (
            <div className="settings-tab-pane">
              <div className="pane-header">
                <div>
                  <h4>Speech-to-Text Transcription</h4>
                  <p>
                    Configure how audio recordings are transcribed into
                    word-level timestamped transcripts.
                  </p>
                </div>
              </div>

              {/* Transcription Engine Select */}
              <div className="settings-grid-options">
                <div
                  className={`settings-option-card ${
                    transcriptionEngine === "deepgram" ? "selected" : ""
                  }`}
                  onClick={() => {
                    setTranscriptionEngine("deepgram");
                    localStorage.setItem("clipon_transcription_engine", "deepgram");
                  }}
                  role="button"
                  tabIndex={0}
                >
                  <div className="option-header">
                    <div className="option-icon-box">
                      <Zap size={18} />
                    </div>
                    <span className="option-badge cloud">Cloud Fast</span>
                  </div>
                  <h5>Deepgram Nova-3 (Cloud)</h5>
                  <p>
                    Ultra-fast cloud transcription with speaker diarization and
                    exact word timestamps. Recommended for fast turnaround.
                  </p>
                </div>

                <div
                  className={`settings-option-card ${
                    transcriptionEngine === "local" ? "selected" : ""
                  }`}
                  onClick={() => {
                    setTranscriptionEngine("local");
                    localStorage.setItem("clipon_transcription_engine", "local");
                  }}
                  role="button"
                  tabIndex={0}
                >
                  <div className="option-header">
                    <div className="option-icon-box">
                      <AudioLines size={18} />
                    </div>
                    <span className="option-badge local">Private On-Device</span>
                  </div>
                  <h5>Whisper Offline (Local)</h5>
                  <p>
                    Transcribes audio locally using whisper.cpp. Completely
                    private with zero API fees. Runs on CPU / Apple Silicon.
                  </p>
                </div>
              </div>

              {/* Deepgram Configuration */}
              <div className="settings-card">
                <SettingsSecretField
                  id="deepgram-key"
                  label="Deepgram API Key"
                  description="Required when using Deepgram Cloud transcription mode."
                  value={deepgramKey}
                  onChange={setDeepgramKey}
                  isSavedInKeystore={Boolean(environment?.hasDeepgramKey)}
                  onDeleteCredential={() => onDeleteCredential("deepgram")}
                  placeholder="token..."
                />
              </div>

              {/* Whisper Offline Status */}
              <div className="settings-card">
                <div className="system-telemetry-row">
                  <div className="telemetry-info">
                    <AudioLines size={20} className="telemetry-icon" />
                    <div>
                      <strong>Offline Whisper Model</strong>
                      <p>
                        {environment?.hasLocalWhisperModel
                          ? "Whisper base model is downloaded and ready for offline use."
                          : "Whisper base model will be loaded locally on initial transcription."}
                      </p>
                    </div>
                  </div>
                  <span
                    className={`status-pill ${
                      environment?.hasLocalWhisperModel ? "active" : "neutral"
                    }`}
                  >
                    {environment?.hasLocalWhisperModel ? "Model Ready" : "Auto-Download"}
                  </span>
                </div>
              </div>
            </div>
          )}

          {/* =================================================================
             3. Video & Processing Tab
             ================================================================= */}
          {activeTab === "video" && (
            <div className="settings-tab-pane">
              <div className="pane-header">
                <div>
                  <h4>Video &amp; Smart Reframe</h4>
                  <p>
                    Configure aspect ratio cropping, active speaker detection,
                    and dynamic editing modifiers.
                  </p>
                </div>
              </div>

              {/* 9:16 Reframe Mode */}
              <div className="settings-field-group">
                <label className="settings-field-label">
                  <span>Default 9:16 Reframe Framing</span>
                </label>
                <div className="settings-grid-options three-col">
                  <div
                    className={`settings-option-card ${
                      reframeMode === "vertical_crop" ? "selected" : ""
                    }`}
                    onClick={() => {
                      setReframeMode("vertical_crop");
                      localStorage.setItem("clipon_reframe_mode", "vertical_crop");
                    }}
                    role="button"
                    tabIndex={0}
                  >
                    <div className="option-header">
                      <h5>Vertical Crop</h5>
                      <span className="format-pill">Center</span>
                    </div>
                    <p>Clean 9:16 center crop. Best for talking head interviews and podcasts.</p>
                  </div>

                  <div
                    className={`settings-option-card ${
                      reframeMode === "smart_face_track" ? "selected" : ""
                    }`}
                    onClick={() => {
                      setReframeMode("smart_face_track");
                      localStorage.setItem("clipon_reframe_mode", "smart_face_track");
                    }}
                    role="button"
                    tabIndex={0}
                  >
                    <div className="option-header">
                      <h5>Smart Face Track</h5>
                      <span className="format-pill">AI Vision</span>
                    </div>
                    <p>Dynamically pans the 9:16 window to follow moving subjects.</p>
                  </div>

                  <div
                    className={`settings-option-card ${
                      reframeMode === "original" ? "selected" : ""
                    }`}
                    onClick={() => {
                      setReframeMode("original");
                      localStorage.setItem("clipon_reframe_mode", "original");
                    }}
                    role="button"
                    tabIndex={0}
                  >
                    <div className="option-header">
                      <h5>Original Aspect</h5>
                      <span className="format-pill">16:9</span>
                    </div>
                    <p>Preserves full widescreen frame with letterboxing.</p>
                  </div>
                </div>
              </div>

              {/* Dynamic Editing Modifiers */}
              <div className="settings-section-divider">
                <span>Dynamic Modifiers</span>
              </div>

              <div className="settings-card">
                <div className="settings-toggle-row">
                  <div className="toggle-info">
                    <strong>Dynamic Punch-Zoom</strong>
                    <p>Applies a subtle 1.12x scale punch-in on key statements to boost visual retention.</p>
                  </div>
                  <label className="switch">
                    <input
                      type="checkbox"
                      checked={punchZoom}
                      onChange={(e) => {
                        setPunchZoom(e.target.checked);
                        localStorage.setItem("clipon_punch_zoom", String(e.target.checked));
                      }}
                    />
                    <span className="slider round" />
                  </label>
                </div>

                <div className="toggle-separator" />

                <div className="settings-toggle-row">
                  <div className="toggle-info">
                    <strong>Silence Removal &amp; Jumpcuts</strong>
                    <p>Trims speech pauses longer than 400ms to eliminate dead air.</p>
                  </div>
                  <label className="switch">
                    <input
                      type="checkbox"
                      checked={removeSilence}
                      onChange={(e) => {
                        setRemoveSilence(e.target.checked);
                        localStorage.setItem("clipon_remove_silence", String(e.target.checked));
                      }}
                    />
                    <span className="slider round" />
                  </label>
                </div>

                <div className="toggle-separator" />

                <div className="settings-toggle-row">
                  <div className="toggle-info">
                    <strong>Studio Audio Compression</strong>
                    <p>Two-pass dynamic normalization with broadcast compression for crisp mobile loudness.</p>
                  </div>
                  <label className="switch">
                    <input
                      type="checkbox"
                      checked={studioAudio}
                      onChange={(e) => {
                        setStudioAudio(e.target.checked);
                        localStorage.setItem("clipon_studio_audio", String(e.target.checked));
                      }}
                    />
                    <span className="slider round" />
                  </label>
                </div>
              </div>

              {/* Active Speaker Detection (ASD) */}
              <div className="settings-section-divider">
                <span>Active Speaker Detection (ASD)</span>
              </div>

              <div className="settings-card">
                <div className="settings-field-group">
                  <div className="settings-field-header">
                    <label className="settings-field-label">
                      <span>NVIDIA Maxine Cloud ASD API</span>
                    </label>
                    <span
                      className={`status-pill ${
                        environment?.hasNvidiaKey ? "active" : "neutral"
                      }`}
                    >
                      {environment?.hasNvidiaKey ? "Maxine Cloud Active" : "Local Energy Fallback"}
                    </span>
                  </div>
                  <p className="settings-field-desc">
                    Connect NVIDIA Maxine cloud endpoints for multi-speaker visual lip-sync
                    detection. If left blank, ClipOn uses fast local energy fallback.
                  </p>
                  <SettingsSecretField
                    id="nvidia-key"
                    label="NVIDIA API Key"
                    value={nvidiaKey}
                    onChange={setNvidiaKey}
                    isSavedInKeystore={Boolean(environment?.hasNvidiaKey)}
                    onDeleteCredential={() => onDeleteCredential("nvidia")}
                    placeholder="nvapi-..."
                  />

                  {setNvidiaFunctionId && (
                    <div className="settings-field-group sub-field">
                      <label htmlFor="nvidia-function-id" className="settings-field-label">
                        <span>NVIDIA Function ID (Optional)</span>
                      </label>
                      <input
                        id="nvidia-function-id"
                        type="text"
                        value={nvidiaFunctionId}
                        onChange={(e) => setNvidiaFunctionId(e.target.value)}
                        placeholder="e.g. 5f48b894-..."
                        className="settings-input"
                      />
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}

          {/* =================================================================
             4. Social & Publishing Tab
             ================================================================= */}
          {activeTab === "social" && (
            <div className="settings-tab-pane">
              <div className="pane-header">
                <div>
                  <h4>Social &amp; Publishing Accounts</h4>
                  <p>
                    Connect Instagram Reels and YouTube Shorts to export and
                    schedule clips with one click.
                  </p>
                </div>
              </div>

              {/* Instagram Reels Integration */}
              <div className="settings-card">
                <div className="social-provider-header">
                  <div className="social-brand-title">
                    <div className="social-icon-box instagram">
                      <Instagram size={18} />
                    </div>
                    <div>
                      <strong>Instagram Reels Publishing</strong>
                      <p>Publish rendered 9:16 vertical clips directly to Instagram Reels or an automation webhook.</p>
                    </div>
                  </div>
                  <span
                    className={`status-pill ${
                      instagramProvider === "graph_api"
                        ? environment?.hasInstagramToken
                          ? "active"
                          : "missing"
                        : instagramWebhookUrl.trim()
                          ? "active"
                          : "missing"
                    }`}
                  >
                    {instagramProvider === "graph_api"
                      ? environment?.hasInstagramToken
                        ? "Connected"
                        : "Not Linked"
                      : instagramWebhookUrl.trim()
                        ? "Webhook Configured"
                        : "Not Configured"}
                  </span>
                </div>

                <div className="settings-field-group">
                  <label className="settings-field-label">
                    <span>Publishing Method</span>
                  </label>
                  <div className="settings-option-grid col-2">
                    <button
                      type="button"
                      className={`settings-option-card compact ${instagramProvider === "graph_api" ? "selected" : ""}`}
                      onClick={() => {
                        setInstagramProvider("graph_api");
                        localStorage.setItem("clipon_instagram_provider", "graph_api");
                      }}
                    >
                      <div className="option-title">Meta Graph API (Direct)</div>
                      <div className="option-desc">Official Meta Graph API publishing to Business / Creator account.</div>
                    </button>
                    <button
                      type="button"
                      className={`settings-option-card compact ${instagramProvider === "webhook" ? "selected" : ""}`}
                      onClick={() => {
                        setInstagramProvider("webhook");
                        localStorage.setItem("clipon_instagram_provider", "webhook");
                      }}
                    >
                      <div className="option-title">Automation Webhook</div>
                      <div className="option-desc">Dispatch video payload to Zapier, Make, n8n, or custom server.</div>
                    </button>
                  </div>
                </div>

                {instagramProvider === "graph_api" ? (
                  <>
                    <div className="settings-field-group">
                      <label htmlFor="ig-account-id" className="settings-field-label">
                        <span>Instagram Business Account ID</span>
                      </label>
                      <input
                        id="ig-account-id"
                        type="text"
                        value={instagramAccountId}
                        onChange={(e) => {
                          setInstagramAccountId(e.target.value);
                          localStorage.setItem("clipon_instagram_account_id", e.target.value);
                        }}
                        placeholder="17841400000000000"
                        className="settings-input"
                      />
                    </div>

                    <SettingsSecretField
                      id="ig-token"
                      label="Meta Graph API Access Token"
                      description="User access token with instagram_basic and instagram_content_publish permissions."
                      value={instagramAccessToken}
                      onChange={setInstagramAccessToken}
                      isSavedInKeystore={Boolean(environment?.hasInstagramToken)}
                      onDeleteCredential={() => onDeleteCredential("instagram")}
                      placeholder="EAAG..."
                    />

                    <div className="social-action-row">
                      <button
                        type="button"
                        className="studio-btn secondary small"
                        onClick={testInstagramConnection}
                        disabled={instagramTesting}
                      >
                        {instagramTesting ? (
                          <Loader2 className="spin" size={13} />
                        ) : (
                          <CheckCircle2 size={13} />
                        )}
                        <span>{instagramTesting ? "Testing..." : "Test Connection"}</span>
                      </button>
                      {instagramTestResult && (
                        <span
                          className={`test-result-pill ${
                            instagramTestResult.success ? "success" : "error"
                          }`}
                        >
                          {instagramTestResult.message}
                        </span>
                      )}
                    </div>
                  </>
                ) : (
                  <div className="settings-field-group">
                    <label htmlFor="ig-webhook-url" className="settings-field-label">
                      <span>Automation Webhook URL</span>
                    </label>
                    <input
                      id="ig-webhook-url"
                      type="url"
                      value={instagramWebhookUrl}
                      onChange={(e) => {
                        setInstagramWebhookUrl(e.target.value);
                        localStorage.setItem("clipon_instagram_webhook_url", e.target.value);
                      }}
                      placeholder="https://hooks.zapier.com/hooks/catch/..."
                      className="settings-input"
                    />
                    <p className="settings-field-desc">
                      When publishing, ClipOn dispatches a POST request with video metadata and clip file location to your automated workflow.
                    </p>
                  </div>
                )}
              </div>

              {/* YouTube Shorts Integration */}
              <div className="settings-card">
                <div className="social-provider-header">
                  <div className="social-brand-title">
                    <div className="social-icon-box youtube">
                      <Youtube size={18} />
                    </div>
                    <div>
                      <strong>YouTube Shorts (OAuth2 Data API v3)</strong>
                      <p>Upload 9:16 clips directly to your channel as YouTube Shorts.</p>
                    </div>
                  </div>
                  <span
                    className={`status-pill ${
                      environment?.hasYoutubeConfig ? "active" : "missing"
                    }`}
                  >
                    {environment?.hasYoutubeConfig ? "Connected" : "Not Linked"}
                  </span>
                </div>

                <div className="settings-field-group">
                  <label htmlFor="yt-client-id" className="settings-field-label">
                    <span>Google OAuth2 Client ID</span>
                  </label>
                  <input
                    id="yt-client-id"
                    type="text"
                    value={youtubeClientId}
                    onChange={(e) => {
                      setYoutubeClientId(e.target.value);
                      localStorage.setItem("clipon_youtube_client_id", e.target.value);
                    }}
                    placeholder="123456789-xxx.apps.googleusercontent.com"
                    className="settings-input"
                  />
                </div>

                <SettingsSecretField
                  id="yt-client-secret"
                  label="OAuth2 Client Secret"
                  value={youtubeClientSecret}
                  onChange={setYoutubeClientSecret}
                  isSavedInKeystore={Boolean(environment?.hasYoutubeConfig)}
                  onDeleteCredential={() => onDeleteCredential("youtube_client_secret")}
                  placeholder="GOCSPX-..."
                />

                <SettingsSecretField
                  id="yt-refresh-token"
                  label="OAuth2 Refresh Token"
                  value={youtubeRefreshToken}
                  onChange={setYoutubeRefreshToken}
                  isSavedInKeystore={Boolean(environment?.hasYoutubeConfig)}
                  onDeleteCredential={() => onDeleteCredential("youtube_refresh_token")}
                  placeholder="1//04..."
                />

                <div className="tos-acknowledgment-box">
                  <label className="checkbox-label">
                    <input
                      type="checkbox"
                      checked={tosAck}
                      onChange={(e) => handleTosToggle(e.target.checked)}
                    />
                    <span>I acknowledge adherence to YouTube API Services Terms of Service</span>
                  </label>
                </div>

                <div className="social-action-row">
                  <button
                    type="button"
                    className="studio-btn secondary small"
                    onClick={testYoutubeConnection}
                    disabled={youtubeTesting}
                  >
                    {youtubeTesting ? (
                      <Loader2 className="spin" size={13} />
                    ) : (
                      <CheckCircle2 size={13} />
                    )}
                    <span>{youtubeTesting ? "Testing..." : "Test Connection"}</span>
                  </button>
                  {youtubeTestResult && (
                    <span
                      className={`test-result-pill ${
                        youtubeTestResult.success ? "success" : "error"
                      }`}
                    >
                      {youtubeTestResult.message}
                    </span>
                  )}
                </div>
              </div>
            </div>
          )}

          {/* =================================================================
             5. Storage & Paths Tab
             ================================================================= */}
          {activeTab === "storage" && (
            <div className="settings-tab-pane">
              <div className="pane-header">
                <div>
                  <h4>Storage &amp; Destinations</h4>
                  <p>
                    Set local directories for downloaded YouTube recordings,
                    cut video clips, and database cache.
                  </p>
                </div>
              </div>

              {/* YouTube Downloads Directory */}
              <div className="settings-card">
                <div className="settings-field-group">
                  <label className="settings-field-label">
                    <span>YouTube Download Staging Directory</span>
                  </label>
                  <p className="settings-field-desc">
                    Where raw videos downloaded via yt-dlp are saved before clipping.
                  </p>
                  <div className="path-display-box">
                    <span className="path-text truncate">
                      {youtubeSaveDir || defaultFolders?.youtubeSaveDir || "Default Downloads"}
                    </span>
                  </div>
                  <div className="path-actions-row">
                    <button
                      type="button"
                      className="studio-btn secondary small"
                      onClick={() =>
                        browseFolder(
                          youtubeSaveDir || defaultFolders?.youtubeSaveDir || "",
                          setYoutubeSaveDir,
                          "clipon_youtube_dir"
                        )
                      }
                    >
                      <Folder size={13} />
                      <span>Browse Folder</span>
                    </button>
                    {youtubeSaveDir && (
                      <>
                        <button
                          type="button"
                          className="studio-btn secondary small"
                          onClick={() => openFolder(youtubeSaveDir)}
                          title="Reveal folder in Finder"
                        >
                          <ExternalLink size={13} />
                          <span>Reveal</span>
                        </button>
                        <button
                          type="button"
                          className="studio-btn secondary small"
                          onClick={() => {
                            setYoutubeSaveDir("");
                            localStorage.removeItem("clipon_youtube_dir");
                          }}
                          title="Reset to default directory"
                        >
                          <RotateCcw size={13} />
                          <span>Reset</span>
                        </button>
                      </>
                    )}
                  </div>
                </div>
              </div>

              {/* Rendered Clips Directory */}
              <div className="settings-card">
                <div className="settings-field-group">
                  <label className="settings-field-label">
                    <span>Rendered Clips Output Directory</span>
                  </label>
                  <p className="settings-field-desc">
                    Where cut 9:16 vertical video clips and subtitle files are rendered.
                  </p>
                  <div className="path-display-box">
                    <span className="path-text truncate">
                      {clipsSaveDir || defaultFolders?.clipsOutputDir || "Default Output"}
                    </span>
                  </div>
                  <div className="path-actions-row">
                    <button
                      type="button"
                      className="studio-btn secondary small"
                      onClick={() =>
                        browseFolder(
                          clipsSaveDir || defaultFolders?.clipsOutputDir || "",
                          setClipsSaveDir,
                          "clipon_clips_dir"
                        )
                      }
                    >
                      <Folder size={13} />
                      <span>Browse Folder</span>
                    </button>
                    {clipsSaveDir && (
                      <>
                        <button
                          type="button"
                          className="studio-btn secondary small"
                          onClick={() => openFolder(clipsSaveDir)}
                          title="Reveal folder in Finder"
                        >
                          <ExternalLink size={13} />
                          <span>Reveal</span>
                        </button>
                        <button
                          type="button"
                          className="studio-btn secondary small"
                          onClick={() => {
                            setClipsSaveDir("");
                            localStorage.removeItem("clipon_clips_dir");
                          }}
                          title="Reset to default directory"
                        >
                          <RotateCcw size={13} />
                          <span>Reset</span>
                        </button>
                      </>
                    )}
                  </div>
                </div>
              </div>

              {/* Application Data Directory */}
              <div className="settings-card">
                <div className="settings-field-group">
                  <label className="settings-field-label">
                    <span>Application SQLite Database &amp; Data Directory</span>
                  </label>
                  <p className="settings-field-desc">
                    Stores projects, dialogue segments, candidates, and secure keystore.
                  </p>
                  <div className="path-display-box">
                    <span className="path-text truncate">
                      {environment?.dataDir || "~/.clipon"}
                    </span>
                  </div>
                  {environment?.dataDir && (
                    <div className="path-actions-row">
                      <button
                        type="button"
                        className="studio-btn secondary small"
                        onClick={() => openFolder(environment.dataDir)}
                      >
                        <ExternalLink size={13} />
                        <span>Open Data Folder</span>
                      </button>
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}

          {/* =================================================================
             6. System Diagnostics Tab
             ================================================================= */}
          {activeTab === "system" && (
            <div className="settings-tab-pane">
              <div className="pane-header">
                <div>
                  <h4>System &amp; Diagnostics</h4>
                  <p>
                    Verify local hardware acceleration, binary tool status,
                    and manage encrypted credentials.
                  </p>
                </div>
              </div>

              {/* Hardware Acceleration Status */}
              <div className="settings-card highlight">
                <div className="system-telemetry-row">
                  <div className="telemetry-info">
                    <Zap size={22} className="telemetry-icon-emerald" />
                    <div>
                      <strong>Apple Silicon VideoToolbox GPU Acceleration</strong>
                      <p>
                        {environment?.hasHardwareAccel
                          ? "Hardware accelerated h264_videotoolbox encoder is active for fast rendering."
                          : "Standard multi-threaded CPU encoding fallback is active."}
                      </p>
                    </div>
                  </div>
                  <span
                    className={`status-pill ${
                      environment?.hasHardwareAccel ? "active" : "neutral"
                    }`}
                  >
                    {environment?.hasHardwareAccel ? "GPU Active" : "CPU Fallback"}
                  </span>
                </div>
              </div>

              {/* Binary Tools Telemetry */}
              <div className="settings-card">
                <h5 className="settings-card-title">Installed Toolchain Diagnostic</h5>
                <div className="binary-diagnostic-grid">
                  <div className="diag-item">
                    <span>FFmpeg Full</span>
                    <span className={`diag-badge ${environment?.hasFfmpeg ? "ok" : "err"}`}>
                      {environment?.hasFfmpeg ? "Detected" : "Missing"}
                    </span>
                  </div>
                  <div className="diag-item">
                    <span>FFprobe</span>
                    <span className={`diag-badge ${environment?.hasFfprobe ? "ok" : "err"}`}>
                      {environment?.hasFfprobe ? "Detected" : "Missing"}
                    </span>
                  </div>
                  <div className="diag-item">
                    <span>yt-dlp (YouTube)</span>
                    <span className={`diag-badge ${environment?.hasYtdlp ? "ok" : "err"}`}>
                      {environment?.hasYtdlp ? "Detected" : "Missing"}
                    </span>
                  </div>
                  <div className="diag-item">
                    <span>whisper.cpp</span>
                    <span className={`diag-badge ${environment?.hasLocalWhisperModel ? "ok" : "warn"}`}>
                      {environment?.hasLocalWhisperModel ? "Ready" : "Standby"}
                    </span>
                  </div>
                  <div className="diag-item">
                    <span>Ollama Daemon</span>
                    <span className={`diag-badge ${environment?.hasOllama ? "ok" : "warn"}`}>
                      {environment?.hasOllama ? "Running" : "Offline"}
                    </span>
                  </div>
                </div>
              </div>

              {/* Keystore Management */}
              <div className="settings-card">
                <h5 className="settings-card-title">Delete Saved Credential</h5>
                <p className="settings-field-desc">
                  Select an individual API key or token to permanently remove from the secure keystore.
                </p>
                <div className="delete-credential-row">
                  <select
                    value={credentialToDelete}
                    onChange={(e) => setCredentialToDelete(e.target.value)}
                    className="settings-select"
                  >
                    <option value="gemini">Google Gemini Key</option>
                    <option value="openai">OpenAI Key</option>
                    <option value="anthropic">Anthropic Key</option>
                    <option value="deepseek">DeepSeek Key</option>
                    <option value="groq">Groq Key</option>
                    <option value="deepgram">Deepgram Key</option>
                    <option value="nvidia">NVIDIA Maxine Key</option>
                    <option value="instagram">Instagram Access Token</option>
                    <option value="youtube">YouTube OAuth Credentials</option>
                  </select>

                  <button
                    type="button"
                    className="studio-btn danger small"
                    onClick={() => onDeleteCredential(credentialToDelete)}
                  >
                    <Trash2 size={13} />
                    <span>Delete Credential</span>
                  </button>
                </div>
              </div>

              {/* Danger Zone */}
              <div className="settings-card danger-zone">
                <div className="danger-zone-header">
                  <AlertTriangle size={18} className="danger-icon" />
                  <div>
                    <strong>Danger Zone: Reset App Storage</strong>
                    <p>
                      Permanently wipes all local project recordings, cut clips,
                      transcripts, and database records.
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  className="studio-btn danger"
                  onClick={onClearStorage}
                >
                  <Trash2 size={14} />
                  <span>Wipe All Project Storage</span>
                </button>
              </div>
            </div>
          )}
        </main>
      </div>

      {/* Sticky Bottom Action Bar */}
      <SettingsFooter
        hasUnsavedChanges={hasUnsavedChanges}
        isSaving={isSaving}
        onCancel={onClose}
        onSave={handleSave}
      />
    </AccessibleModal>
  );
}
