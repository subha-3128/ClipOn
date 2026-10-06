import React, { useEffect, useMemo, useState, useRef } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import {
  Clapperboard,
  FileVideo,
  Youtube,
  Sparkles,
  Scissors,
  FolderOpen,
  Settings,
  SlidersHorizontal,
  RefreshCw,
  Play,
  Check,
  Copy,
  Download,
  Loader2,
  ChevronRight,
  Search,
  X,
  AlertTriangle,
  BadgeCheck,
  AudioLines,
  Captions,
  Cpu,
  Layers,
  Trash2,
  Edit3,
  ArrowLeft,
  Database,
  Cloud,
  ExternalLink,
  Sliders,
  Filter,
  CheckSquare,
  Square,
  Instagram,
  Flame,
  Hash,
  Zap,
  Mic,
  Monitor,
  Activity,
  CheckCircle2,
  Menu,
  Users
} from "lucide-react";
import "./styles.css";

// ===== TypeScript Types =====
export type EnvironmentStatus = {
  dataDir: string;
  hasFfmpeg: boolean;
  hasFfprobe: boolean;
  hasDeepgramKey: boolean;
  hasAnthropicKey: boolean;
  hasDeepseekKey: boolean;
  hasGeminiKey: boolean;
  hasOpenaiKey: boolean;
  hasOpenrouterKey: boolean;
  hasGroqKey: boolean;
  llmProvider: string;
  hasLocalWhisperModel: boolean;
  hasOllama: boolean;
  hasYtdlp: boolean;
  hasHardwareAccel?: boolean;
  deepgramKey?: string;
  geminiKey?: string;
  deepseekKey?: string;
  anthropicKey?: string;
  groqKey?: string;
  openaiKey?: string;
  openrouterKey?: string;
  instagramAccountId?: string;
  instagramAccessToken?: string;
};

export type Project = {
  id: string;
  name: string | null;
  sourcePath: string;
  sourceDuration: number | null;
  status: string;
  transcriptionMode: string;
  captionStyle?: string | null;
  createdAt: string;
  updatedAt: string;
};

export type Transcript = {
  id: string;
  projectId: string;
  engine: string;
  rawJson: string;
  language: string | null;
  createdAt: string;
};

export type SocialKit = {
  candidateId: string;
  titles: string[];
  description: string;
  hashtags: string[];
  callToAction: string;
};

export type Candidate = {
  id: string;
  projectId: string;
  startSec: number;
  endSec: number;
  score: number;
  hook: string;
  rationale: string;
  rank: number;
  selected: boolean;
};

export type Clip = {
  id: string;
  candidateId: string;
  status: string;
  outputPath: string | null;
  faceTrackJson: string | null;
  captionAssPath: string | null;
  renderLog: string | null;
};

export type InstagramPost = {
  id: string;
  candidateId: string;
  clipId: string | null;
  status: "queued" | "publishing" | "published" | "failed";
  caption: string | null;
  postUrl: string | null;
  errorMessage: string | null;
  createdAt: string;
  publishedAt: string | null;
};

export type ProjectDetail = {
  project: Project;
  transcript: Transcript | null;
  candidates: Candidate[];
  clips: Clip[];
  instagramPosts?: InstagramPost[];
};

export type NormalizedTranscriptSegment = {
  start: number;
  end: number;
  speaker: string | null;
  text: string;
};

export type NormalizedTranscript = {
  language: string;
  duration: number;
  speakers: string[];
  segments: NormalizedTranscriptSegment[];
};

export type BusyState =
  | "idle"
  | "import"
  | "transcribe"
  | "demoTranscript"
  | "moments"
  | "clipCount"
  | "cut";

export type ReframeMode = "vertical_crop" | "podcast_split" | "original";
export type AppSection = "shorts" | "podcast";
export type SettingsTab = "ai" | "storage" | "export" | "system";

// ===== Utility Helpers =====
function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function formatTime(seconds: number): string {
  if (isNaN(seconds) || seconds < 0) return "0:00";
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}

function formatDate(dateStr: string): string {
  try {
    const d = new Date(dateStr);
    return d.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
  } catch {
    return dateStr;
  }
}

// ===== Toast Notification Component =====
function Toast({ message, onClose }: { message: string | null; onClose: () => void }) {
  useEffect(() => {
    if (!message) return;
    const timer = setTimeout(onClose, 2400);
    return () => clearTimeout(timer);
  }, [message, onClose]);

  if (!message) return null;
  return (
    <div className="toast-notification">
      <Check size={15} />
      <span>{message}</span>
    </div>
  );
}

// ===== Main App Component =====
function App() {
  const [environment, setEnvironment] = useState<EnvironmentStatus | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [detail, setDetail] = useState<ProjectDetail | null>(null);
  const [busy, setBusy] = useState<BusyState>("idle");
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  // Modals state
  const [showSettings, setShowSettings] = useState(false);
  const [settingsTab, setSettingsTab] = useState<SettingsTab>("ai");
  const [showStyleModal, setShowStyleModal] = useState(false);
  const [selectedStyle, setSelectedStyle] = useState("modern-box");
  const [mediaPathToImport, setMediaPathToImport] = useState<string | null>(null);

  const [youtubeModalOpen, setYoutubeModalOpen] = useState(false);
  const [youtubeUrl, setYoutubeUrl] = useState("");
  const [youtubeStatus, setYoutubeStatus] = useState<"idle" | "checking" | "warning" | "downloading">("idle");
  const [youtubeWarningLicense, setYoutubeWarningLicense] = useState<string | null>(null);

  // Candidate cut & social kit state
  const [renderingCandidateId, setRenderingCandidateId] = useState<string | null>(null);
  const [socialKitModalCandidate, setSocialKitModalCandidate] = useState<Candidate | null>(null);
  const [socialKitData, setSocialKitData] = useState<Record<string, SocialKit>>({});
  const [socialKitLoading, setSocialKitLoading] = useState<string | null>(null);

  // Ollama model download state
  const [downloadingModelName, setDownloadingModelName] = useState<string | null>(null);
  const [modelDownloadStatus, setModelDownloadStatus] = useState("");
  const [modelDownloadProgress, setModelDownloadProgress] = useState(0);

  // Settings & persistence
  const [isOnboarded, setIsOnboarded] = useState<boolean | null>(null);
  const [transcriptionEngine, setTranscriptionEngine] = useState<"deepgram" | "local">(() => {
    return ((localStorage.getItem("clipon_transcription_engine") || localStorage.getItem("autoshorts_transcription_engine")) as "deepgram" | "local") || "local";
  });
  const [llmEngine, setLlmEngine] = useState<"claude" | "deepseek" | "local" | "gemini" | "openai" | "openrouter" | "groq">(() => {
    return ((localStorage.getItem("clipon_llm_engine") || localStorage.getItem("autoshorts_llm_engine")) as any) || "local";
  });
  const [localLlmModel, setLocalLlmModel] = useState(() => {
    return (localStorage.getItem("clipon_local_llm_model") || localStorage.getItem("autoshorts_local_llm_model")) || "llama3.2";
  });
  const [deepgramKey, setDeepgramKey] = useState(() => (localStorage.getItem("clipon_deepgram_key") || localStorage.getItem("autoshorts_deepgram_key")) || "");
  const [anthropicKey, setAnthropicKey] = useState(() => (localStorage.getItem("clipon_anthropic_key") || localStorage.getItem("autoshorts_anthropic_key")) || "");
  const [deepseekKey, setDeepseekKey] = useState(() => (localStorage.getItem("clipon_deepseek_key") || localStorage.getItem("autoshorts_deepseek_key")) || "");
  const [deepseekModel, setDeepseekModel] = useState(() => (localStorage.getItem("clipon_deepseek_model") || localStorage.getItem("autoshorts_deepseek_model")) || "");
  const [geminiKey, setGeminiKey] = useState(() => {
    let k = (localStorage.getItem("clipon_gemini_key") || localStorage.getItem("autoshorts_gemini_key")) || "";
    if (k.startsWith("Q.Ab8")) k = "A" + k;
    return k;
  });
  const [openaiKey, setOpenaiKey] = useState(() => (localStorage.getItem("clipon_openai_key") || localStorage.getItem("autoshorts_openai_key")) || "");
  const [openrouterKey, setOpenrouterKey] = useState(() => (localStorage.getItem("clipon_openrouter_key") || localStorage.getItem("autoshorts_openrouter_key")) || "");
  const [openrouterModel, setOpenrouterModel] = useState(() => (localStorage.getItem("clipon_openrouter_model") || localStorage.getItem("autoshorts_openrouter_model")) || "");
  const [groqKey, setGroqKey] = useState(() => (localStorage.getItem("clipon_groq_key") || localStorage.getItem("autoshorts_groq_key")) || "");

  // Folder paths
  const [youtubeSaveDir, setYoutubeSaveDir] = useState(() => (localStorage.getItem("clipon_youtube_dir") || localStorage.getItem("autoshorts_youtube_dir")) || "");
  const [clipsSaveDir, setClipsSaveDir] = useState(() => (localStorage.getItem("clipon_clips_dir") || localStorage.getItem("autoshorts_clips_dir")) || "");
  const [defaultFolders, setDefaultFolders] = useState<{ youtubeSaveDir: string; clipsOutputDir: string } | null>(null);

  // Reframe Mode & Modifiers
  const [removeSilence, setRemoveSilence] = useState<boolean>(() => {
    return localStorage.getItem("clipon_remove_silence") === "true";
  });

  const [punchZoom, setPunchZoom] = useState<boolean>(() => {
    const saved = localStorage.getItem("clipon_punch_zoom");
    if (saved !== null) return saved === "true";
    const oldMode = localStorage.getItem("clipon_reframe_mode");
    return oldMode === "punch_zoom";
  });

  const [studioAudio, setStudioAudio] = useState<boolean>(() => {
    return localStorage.getItem("clipon_studio_audio") !== "false";
  });

  const [appSection, setAppSection] = useState<AppSection>(() => {
    const saved = localStorage.getItem("clipon_app_section");
    return saved === "podcast" ? "podcast" : "shorts";
  });

  const [reframeMode, setReframeMode] = useState<ReframeMode>(() => {
    const saved = (localStorage.getItem("clipon_reframe_mode") || localStorage.getItem("autoshorts_reframe_mode")) as any;
    if (saved === "podcast_split" || saved === "original" || saved === "vertical_crop") {
      return saved as ReframeMode;
    }
    return "vertical_crop";
  });

  const handleSectionChange = (section: AppSection) => {
    setAppSection(section);
    localStorage.setItem("clipon_app_section", section);
    if (section === "podcast") {
      setReframeMode("podcast_split");
      localStorage.setItem("clipon_reframe_mode", "podcast_split");
    } else if (reframeMode === "podcast_split") {
      setReframeMode("vertical_crop");
      localStorage.setItem("clipon_reframe_mode", "vertical_crop");
    }
  };

  // Instagram Reels API State
  const [instagramProvider, setInstagramProvider] = useState<"graph_api" | "webhook">(() => {
    return (localStorage.getItem("clipon_instagram_provider") as any) || "graph_api";
  });
  const [instagramAccountId, setInstagramAccountId] = useState(() => {
    return localStorage.getItem("clipon_instagram_account_id") || "";
  });
  const [instagramAccessToken, setInstagramAccessToken] = useState(() => {
    return localStorage.getItem("clipon_instagram_access_token") || "";
  });
  const [instagramWebhookUrl, setInstagramWebhookUrl] = useState(() => {
    return localStorage.getItem("clipon_instagram_webhook_url") || "";
  });
  const [instagramTesting, setInstagramTesting] = useState(false);
  const [instagramTestResult, setInstagramTestResult] = useState<{ success: boolean; message: string } | null>(null);
  const [publishingCandidateId, setPublishingCandidateId] = useState<string | null>(null);

  // Meta Graph API Quick Connect Modal State
  const [showMetaModal, setShowMetaModal] = useState(false);
  const [pendingCandidateIdToPost, setPendingCandidateIdToPost] = useState<string | null>(null);
  const [metaModalAccountId, setMetaModalAccountId] = useState('');
  const [metaModalAccessToken, setMetaModalAccessToken] = useState('');
  const [metaModalSaving, setMetaModalSaving] = useState(false);
  const [metaModalTesting, setMetaModalTesting] = useState(false);
  const [metaModalStatus, setMetaModalStatus] = useState<{ success: boolean; message: string } | null>(null);

  useEffect(() => { localStorage.setItem("clipon_instagram_provider", instagramProvider); }, [instagramProvider]);
  useEffect(() => { localStorage.setItem("clipon_instagram_account_id", instagramAccountId); }, [instagramAccountId]);
  useEffect(() => { localStorage.setItem("clipon_instagram_access_token", instagramAccessToken); }, [instagramAccessToken]);
  useEffect(() => { localStorage.setItem("clipon_instagram_webhook_url", instagramWebhookUrl); }, [instagramWebhookUrl]);

  // UI Filters
  const [projectSearch, setProjectSearch] = useState("");
  const [transcriptSearch, setTranscriptSearch] = useState("");
  const [momentTab, setMomentTab] = useState<"all" | "selected" | "ready">("all");

  const showToast = (msg: string) => setToast(msg);

  function getActiveLlmKey(): string {
    let key =
      llmEngine === "claude" ? (anthropicKey.trim() || environment?.anthropicKey || "") :
      llmEngine === "deepseek" ? (deepseekKey.trim() || environment?.deepseekKey || "") :
      llmEngine === "gemini" ? (geminiKey.trim() || environment?.geminiKey || "") :
      llmEngine === "openai" ? (openaiKey.trim() || environment?.openaiKey || "") :
      llmEngine === "openrouter" ? (openrouterKey.trim() || environment?.openrouterKey || "") :
      llmEngine === "groq" ? (groqKey.trim() || environment?.groqKey || "") : "";
    if (llmEngine === "gemini" && key.startsWith("Q.Ab8")) {
      key = "A" + key;
    }
    return key;
  }

  // Sync state with LocalStorage
  useEffect(() => { localStorage.setItem("clipon_transcription_engine", transcriptionEngine); }, [transcriptionEngine]);
  useEffect(() => { localStorage.setItem("clipon_llm_engine", llmEngine); }, [llmEngine]);
  useEffect(() => { localStorage.setItem("clipon_local_llm_model", localLlmModel); }, [localLlmModel]);
  useEffect(() => { localStorage.setItem("clipon_deepgram_key", deepgramKey); }, [deepgramKey]);
  useEffect(() => { localStorage.setItem("clipon_anthropic_key", anthropicKey); }, [anthropicKey]);
  useEffect(() => { localStorage.setItem("clipon_deepseek_key", deepseekKey); }, [deepseekKey]);
  useEffect(() => { localStorage.setItem("clipon_deepseek_model", deepseekModel); }, [deepseekModel]);
  useEffect(() => { localStorage.setItem("clipon_gemini_key", geminiKey); }, [geminiKey]);
  useEffect(() => { localStorage.setItem("clipon_openai_key", openaiKey); }, [openaiKey]);
  useEffect(() => { localStorage.setItem("clipon_openrouter_key", openrouterKey); }, [openrouterKey]);
  useEffect(() => { localStorage.setItem("clipon_openrouter_model", openrouterModel); }, [openrouterModel]);
  useEffect(() => { localStorage.setItem("clipon_groq_key", groqKey); }, [groqKey]);
  useEffect(() => { localStorage.setItem("clipon_youtube_dir", youtubeSaveDir); }, [youtubeSaveDir]);
  useEffect(() => { localStorage.setItem("clipon_clips_dir", clipsSaveDir); }, [clipsSaveDir]);
  useEffect(() => { localStorage.setItem("clipon_reframe_mode", reframeMode); }, [reframeMode]);
  useEffect(() => { localStorage.setItem("clipon_punch_zoom", String(punchZoom)); }, [punchZoom]);
  useEffect(() => { localStorage.setItem("clipon_studio_audio", String(studioAudio)); }, [studioAudio]);

  // Initial load
  useEffect(() => {
    void refresh();
    const onboardedVal = localStorage.getItem("clipon_onboarded") || localStorage.getItem("autoshorts_onboarded");
    setIsOnboarded(onboardedVal === "true");

    // Load default folders
    invoke<{ youtube_download_dir: string; clips_output_dir: string }>("get_default_folders")
      .then((dirs) => setDefaultFolders({ youtubeSaveDir: dirs.youtube_download_dir, clipsOutputDir: dirs.clips_output_dir }))
      .catch(() => {});
  }, []);

  // Listen for Escape key to close modals
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setShowSettings(false);
        setYoutubeModalOpen(false);
        setShowStyleModal(false);
        setSocialKitModalCandidate(null);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  // Refresh data from Rust backend
  async function refresh(nextProjectId?: string) {
    setError(null);
    try {
      const [env, projectList] = await Promise.all([
        invoke<EnvironmentStatus>("environment_status"),
        invoke<Project[]>("list_projects"),
      ]);
      setEnvironment(env);
      setProjects(projectList);

      if (env.hasDeepgramKey && !localStorage.getItem("clipon_transcription_engine")) {
        setTranscriptionEngine("deepgram");
      }
      if (env.deepgramKey && (!deepgramKey || !localStorage.getItem("clipon_deepgram_key"))) {
        setDeepgramKey(env.deepgramKey);
        localStorage.setItem("clipon_deepgram_key", env.deepgramKey);
      }
      if (env.geminiKey && (!geminiKey || geminiKey.startsWith("Q.Ab8") || !localStorage.getItem("clipon_gemini_key"))) {
        setGeminiKey(env.geminiKey);
        localStorage.setItem("clipon_gemini_key", env.geminiKey);
      }
      if (env.deepseekKey && (!deepseekKey || !localStorage.getItem("clipon_deepseek_key"))) {
        setDeepseekKey(env.deepseekKey);
        localStorage.setItem("clipon_deepseek_key", env.deepseekKey);
      }
      if (env.anthropicKey && (!anthropicKey || !localStorage.getItem("clipon_anthropic_key"))) {
        setAnthropicKey(env.anthropicKey);
        localStorage.setItem("clipon_anthropic_key", env.anthropicKey);
      }
      if (env.groqKey && (!groqKey || !localStorage.getItem("clipon_groq_key"))) {
        setGroqKey(env.groqKey);
        localStorage.setItem("clipon_groq_key", env.groqKey);
      }
      if (env.instagramAccountId && (!instagramAccountId || !localStorage.getItem("clipon_instagram_account_id"))) {
        setInstagramAccountId(env.instagramAccountId);
        localStorage.setItem("clipon_instagram_account_id", env.instagramAccountId);
      }
      if (env.instagramAccessToken && (!instagramAccessToken || !localStorage.getItem("clipon_instagram_access_token"))) {
        setInstagramAccessToken(env.instagramAccessToken);
        localStorage.setItem("clipon_instagram_access_token", env.instagramAccessToken);
      }

      if (env.hasDeepgramKey || env.hasGeminiKey || env.hasDeepseekKey || env.hasAnthropicKey || env.hasGroqKey || env.hasLocalWhisperModel || env.hasOllama || projectList.length > 0) {
        setIsOnboarded(true);
        localStorage.setItem("clipon_onboarded", "true");
      }

      if (nextProjectId) {
        const nextDetail = await invoke<ProjectDetail>("get_project_detail", { projectId: nextProjectId });
        setDetail(nextDetail);
      } else if (detail) {
        const nextDetail = await invoke<ProjectDetail>("get_project_detail", { projectId: detail.project.id });
        setDetail(nextDetail);
      }
    } catch (err) {
      console.error("Refresh error:", err);
    }
  }

  // Derived transcript & clips
  const transcript = useMemo(() => {
    if (!detail?.transcript) return null;
    try {
      return JSON.parse(detail.transcript.rawJson) as NormalizedTranscript;
    } catch {
      return null;
    }
  }, [detail?.transcript]);

  const clipByCandidate = useMemo(() => {
    return new Map(detail?.clips.map((clip) => [clip.candidateId, clip]) ?? []);
  }, [detail?.clips]);

  const instagramPostByCandidate = useMemo(() => {
    return new Map((detail?.instagramPosts ?? []).map((post) => [post.candidateId, post]));
  }, [detail?.instagramPosts]);

  async function testInstagramConnection() {
    setInstagramTesting(true);
    setInstagramTestResult(null);
    try {
      const msg = await invoke<string>("test_instagram_connection", {
        provider: instagramProvider,
        accountId: instagramAccountId.trim() || null,
        accessToken: instagramAccessToken.trim() || null,
        webhookUrl: instagramWebhookUrl.trim() || null,
      });
      setInstagramTestResult({ success: true, message: msg });
      showToast("Instagram connected!");
    } catch (err) {
      setInstagramTestResult({ success: false, message: String(err) });
    } finally {
      setInstagramTesting(false);
    }
  }

  async function handlePublishToInstagram(candidateId: string) {
    if (!detail) return;
    const currentAccId = (
      instagramAccountId.trim() ||
      environment?.instagramAccountId?.trim() ||
      ""
    );
    const currentToken = (
      instagramAccessToken.trim() ||
      environment?.instagramAccessToken?.trim() ||
      ""
    );

    if (instagramProvider === "graph_api" && (!currentAccId || !currentToken)) {
      setSettingsTab("ai");
      setShowSettings(true);
      showToast("Configure your Instagram Reels API credentials in Settings");
      return;
    }
    if (instagramProvider === "webhook" && !instagramWebhookUrl.trim()) {
      setSettingsTab("ai");
      setShowSettings(true);
      showToast("Configure your Instagram Webhook URL in Settings");
      return;
    }

    await executeInstagramPublish(candidateId, currentAccId, currentToken);
  }

  async function executeInstagramPublish(candidateId: string, accId?: string, token?: string) {
    if (!detail) return;
    setPublishingCandidateId(candidateId);
    try {
      const activeAccId = accId || instagramAccountId.trim() || environment?.instagramAccountId?.trim() || null;
      const activeToken = token || instagramAccessToken.trim() || environment?.instagramAccessToken?.trim() || null;

      await invoke<InstagramPost>("publish_candidate_to_instagram", {
        candidateId,
        captionOverride: null,
        provider: instagramProvider,
        accountId: activeAccId,
        accessToken: activeToken,
        webhookUrl: instagramWebhookUrl.trim() || null,
      });
      await refresh(detail.project.id);
      showToast("🎉 Successfully published to Instagram Reels!");
    } catch (err) {
      console.error("Instagram publish error:", err);
      setError(String(err));
      showToast("Instagram error: " + String(err));
    } finally {
      setPublishingCandidateId(null);
    }
  }

  async function handleSaveMetaAndPost() {
    if (!metaModalAccountId.trim()) {
      showToast("Please enter your Instagram Account ID");
      return;
    }
    if (!metaModalAccessToken.trim()) {
      showToast("Please enter your Meta Graph API Access Token");
      return;
    }
    setMetaModalSaving(true);
    try {
      setInstagramAccountId(metaModalAccountId.trim());
      setInstagramAccessToken(metaModalAccessToken.trim());
      localStorage.setItem("clipon_instagram_account_id", metaModalAccountId.trim());
      localStorage.setItem("clipon_instagram_access_token", metaModalAccessToken.trim());

      await invoke("save_instagram_credentials", {
        accountId: metaModalAccountId.trim(),
        accessToken: metaModalAccessToken.trim(),
      });

      showToast("Meta Graph API credentials saved!");
      setShowMetaModal(false);

      if (pendingCandidateIdToPost) {
        const candId = pendingCandidateIdToPost;
        setPendingCandidateIdToPost(null);
        await executeInstagramPublish(candId, metaModalAccountId.trim(), metaModalAccessToken.trim());
      }
    } catch (err) {
      showToast("Failed to save credentials: " + String(err));
    } finally {
      setMetaModalSaving(false);
    }
  }

  async function handleTestMetaConnection() {
    if (!metaModalAccountId.trim() || !metaModalAccessToken.trim()) {
      setMetaModalStatus({ success: false, message: "Please enter both Account ID and Access Token to test" });
      return;
    }
    setMetaModalTesting(true);
    setMetaModalStatus(null);
    try {
      const msg = await invoke<string>("test_instagram_connection", {
        provider: "graph_api",
        accountId: metaModalAccountId.trim(),
        accessToken: metaModalAccessToken.trim(),
        webhookUrl: null,
      });
      setMetaModalStatus({ success: true, message: msg });
    } catch (err) {
      setMetaModalStatus({ success: false, message: String(err) });
    } finally {
      setMetaModalTesting(false);
    }
  }

  const selectedCount = detail?.candidates.filter((c) => c.selected).length ?? 0;
  const cutCount = detail?.candidates.filter((c) => {
    const clip = clipByCandidate.get(c.id);
    return clip?.status === "done" && Boolean(clip.outputPath);
  }).length ?? 0;


  const canUseCloudKey = Boolean(environment?.hasDeepgramKey || deepgramKey.trim().length > 0);
  const canUseClaude = Boolean(environment?.hasAnthropicKey || anthropicKey.trim().length > 0);
  const canUseDeepseek = Boolean(environment?.hasDeepseekKey || deepseekKey.trim().length > 0);
  const canUseGemini = Boolean(environment?.hasGeminiKey || geminiKey.trim().length > 0);
  const canUseOpenai = Boolean(environment?.hasOpenaiKey || openaiKey.trim().length > 0);
  const canUseOpenrouter = Boolean(environment?.hasOpenrouterKey || openrouterKey.trim().length > 0);
  const canUseGroq = Boolean(environment?.hasGroqKey || groqKey.trim().length > 0);

  const canTranscribe = transcriptionEngine === "local"
    ? Boolean(environment?.hasLocalWhisperModel)
    : canUseCloudKey;

  const canUseActiveLlm = llmEngine === "local"
    ? Boolean(environment?.hasOllama)
    : llmEngine === "claude" ? canUseClaude
    : llmEngine === "deepseek" ? canUseDeepseek
    : llmEngine === "gemini" ? canUseGemini
    : llmEngine === "openai" ? canUseOpenai
    : llmEngine === "openrouter" ? canUseOpenrouter
    : llmEngine === "groq" ? canUseGroq : false;

  async function run(action: BusyState, task: () => Promise<void>) {
    setBusy(action);
    setError(null);
    try {
      await task();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("idle");
    }
  }

  // Open directory in native macOS Finder
  async function openFolder(path: string) {
    try {
      await invoke("open_folder", { path });
    } catch (err) {
      setError(String(err));
    }
  }

  // Pick folder using native dialog
  async function browseFolder(
    currentVal: string,
    setter: (val: string) => void,
    storageKey: string
  ) {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        defaultPath: currentVal || undefined,
      });
      if (typeof selected === "string" && selected.trim()) {
        setter(selected.trim());
        localStorage.setItem(storageKey, selected.trim());
        showToast("Storage location updated");
      }
    } catch (err) {
      console.error("Failed to select folder:", err);
    }
  }

  // Pull Ollama model
  const pullModelDirectly = async (modelName: string) => {
    setDownloadingModelName(modelName);
    setModelDownloadProgress(0);
    setModelDownloadStatus("Connecting to Ollama...");
    try {
      const unlisten = await listen<{
        status: string;
        completed?: number;
        total?: number;
        percentage?: number;
      }>("ollama-pull-progress", (event) => {
        const payload = event.payload;
        setModelDownloadStatus(payload.status);
        if (payload.percentage !== undefined && payload.percentage !== null) {
          setModelDownloadProgress(Math.round(payload.percentage));
        }
      });

      await invoke("pull_ollama_model", { modelName });
      unlisten();
      setModelDownloadStatus("Download complete!");
      setModelDownloadProgress(100);
      showToast(`Model ${modelName} downloaded!`);
      setTimeout(() => setDownloadingModelName(null), 600);
    } catch (err) {
      alert("Failed to download model: " + String(err));
      setDownloadingModelName(null);
    }
  };

  // Import local media
  async function importMedia() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Media", extensions: ["mp4", "mov", "mp3", "wav", "m4a"] }],
    });
    if (typeof selected !== "string") return;
    setMediaPathToImport(selected);
    setShowStyleModal(true);
  }

  // YouTube import flow
  async function handleYoutubeImport() {
    if (!youtubeUrl) return;
    setYoutubeStatus("checking");
    setError(null);
    try {
      const result = await invoke<{ isSafe: boolean; license: string | null }>("check_youtube_copyright", { url: youtubeUrl });
      if (!result.isSafe) {
        setYoutubeWarningLicense(result.license || "Standard YouTube License");
        setYoutubeStatus("warning");
        return;
      }
      await executeYoutubeDownload();
    } catch (err: any) {
      setError(err.toString());
      setYoutubeStatus("idle");
    }
  }

  async function executeYoutubeDownload() {
    setYoutubeStatus("downloading");
    setError(null);
    try {
      const downloadedPath = await invoke<string>("download_youtube_video", {
        url: youtubeUrl,
        outputDir: youtubeSaveDir.trim() || null,
      });
      setYoutubeModalOpen(false);
      setYoutubeUrl("");
      setYoutubeStatus("idle");
      setMediaPathToImport(downloadedPath);
      setShowStyleModal(true);
    } catch (err: any) {
      setError(err.toString());
      setYoutubeStatus("idle");
    }
  }

  async function confirmImport(style: string) {
    if (!mediaPathToImport) return;
    const selected = mediaPathToImport;
    setMediaPathToImport(null);
    setShowStyleModal(false);

    let newProjectId: string | null = null;
    await run("import", async () => {
      const project = await invoke<Project>("create_project_from_path", {
        path: selected,
        transcriptionMode: transcriptionEngine === "local" ? "local" : "cloud",
        captionStyle: style,
      });
      newProjectId = project.id;
      await refresh(project.id);
      showToast("Media imported successfully!");
    });

    if (newProjectId) {
      await runAutoPipeline(newProjectId);
    }
  }

  // Automated pipeline
  async function runAutoPipeline(projectId: string) {
    setError(null);
    const env = await invoke<EnvironmentStatus>("environment_status");

    if (transcriptionEngine === "local" && !env.hasLocalWhisperModel) {
      setError("Import complete. Local Whisper is missing. Add model or switch to Cloud in Settings.");
      return;
    }
    if (transcriptionEngine === "deepgram" && !canUseCloudKey) {
      setError("Import complete. Deepgram API Key is missing. Add it in Settings to transcribe.");
      return;
    }

    // 1. Transcribe
    try {
      setBusy("transcribe");
      await invoke<Transcript>("transcribe_project", {
        projectId,
        provider: transcriptionEngine,
        apiKey: transcriptionEngine === "deepgram" ? (deepgramKey.trim() || null) : null,
      });
      await refresh(projectId);
      showToast("Transcription complete!");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setBusy("idle");
      return;
    }

    // 2. Moments detection
    try {
      setBusy("moments");
      const activeKey = getActiveLlmKey();

      await invoke<Candidate[]>("generate_candidates", {
        projectId,
        apiKey: activeKey || null,
        provider: llmEngine,
        modelName: llmEngine === "local" ? localLlmModel.trim() : (llmEngine === "deepseek" ? (deepseekModel.trim() || null) : (llmEngine === "openrouter" ? (openrouterModel.trim() || null) : null)),
        allowDemo: false,
      });
      await refresh(projectId);
      showToast("Viral moments detected!");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("idle");
    }
  }

  // Project management
  async function selectProject(projectId: string) {
    await run("idle", async () => {
      const nextDetail = await invoke<ProjectDetail>("get_project_detail", { projectId });
      setDetail(nextDetail);
    });
  }

  async function renameProject(projectId: string) {
    const project = projects.find((p) => p.id === projectId);
    if (!project) return;
    const currentName = project.name || fileName(project.sourcePath);
    const newName = window.prompt("Rename Project:", currentName);
    if (newName === null) return;
    const trimmed = newName.trim();
    if (!trimmed) return;

    try {
      await invoke("rename_project", { projectId, name: trimmed });
      await refresh(detail?.project.id);
      showToast("Project renamed");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function deleteProject(projectId: string) {
    const project = projects.find((p) => p.id === projectId);
    if (!project) return;
    const name = project.name || fileName(project.sourcePath);
    if (!window.confirm(`Delete project "${name}"? This cannot be undone.`)) return;

    try {
      await invoke("delete_project", { projectId });
      if (detail?.project.id === projectId) setDetail(null);
      await refresh();
      showToast("Project deleted");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  // Manual actions in project workspace
  async function transcribe() {
    if (!detail) return;
    await run("transcribe", async () => {
      await invoke<Transcript>("transcribe_project", {
        projectId: detail.project.id,
        provider: transcriptionEngine,
        apiKey: transcriptionEngine === "deepgram" ? (deepgramKey.trim() || null) : null,
      });
      await refresh(detail.project.id);
      showToast("Transcription finished");
    });
  }

  async function moments() {
    if (!detail) return;
    await run("moments", async () => {
      const activeKey = getActiveLlmKey();

      await invoke<Candidate[]>("generate_candidates", {
        projectId: detail.project.id,
        apiKey: activeKey || null,
        provider: llmEngine,
        modelName: llmEngine === "local" ? localLlmModel.trim() : (llmEngine === "deepseek" ? (deepseekModel.trim() || null) : (llmEngine === "openrouter" ? (openrouterModel.trim() || null) : null)),
        allowDemo: false,
      });
      await refresh(detail.project.id);
      showToast("Viral moments detected");
    });
  }

  async function updateClipCount(count: number) {
    if (!detail) return;
    await run("clipCount", async () => {
      const candidates = await invoke<Candidate[]>("set_selected_clip_count", {
        projectId: detail.project.id,
        count,
      });
      setDetail({ ...detail, candidates });
    });
  }

  async function toggleCandidate(candidateId: string) {
    if (!detail) return;
    const target = detail.candidates.find((c) => c.id === candidateId);
    if (!target) return;
    const newSelected = !target.selected;

    const newCandidates = detail.candidates.map((c) => c.id === candidateId ? { ...c, selected: newSelected } : c);
    setDetail({ ...detail, candidates: newCandidates });

    const newCount = newCandidates.filter((c) => c.selected).length;
    await updateClipCount(newCount);
  }

  async function selectBatch(type: "top3" | "top5" | "all" | "none") {
    if (!detail) return;
    const total = detail.candidates.length;
    let count = 0;
    if (type === "top3") count = Math.min(3, total);
    else if (type === "top5") count = Math.min(5, total);
    else if (type === "all") count = total;
    else if (type === "none") count = 0;

    await updateClipCount(count);
  }

  async function cutCandidate(candidateId: string) {
    if (!detail) return;
    setRenderingCandidateId(candidateId);
    setBusy("cut");
    setError(null);
    try {
      await invoke<string>("render_flat_clip_for_candidate", { candidateId, reframeMode, outputDir: clipsSaveDir.trim() || null, removeSilence, punchZoom, studioAudio });
      showToast("Clip rendered successfully!");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setRenderingCandidateId(null);
      setBusy("idle");
      await refresh(detail.project.id);
    }
  }

  async function cutSelected() {
    if (!detail) return;
    const selected = detail.candidates.filter((c) => c.selected);
    if (selected.length === 0) return;

    setBusy("cut");
    setError(null);
    try {
      for (const candidate of selected) {
        setRenderingCandidateId(candidate.id);
        await invoke<string>("render_flat_clip_for_candidate", { candidateId: candidate.id, reframeMode, outputDir: clipsSaveDir.trim() || null, removeSilence, punchZoom, studioAudio });
      }
      showToast(`Finished rendering ${selected.length} clips!`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setRenderingCandidateId(null);
      setBusy("idle");
      await refresh(detail.project.id);
    }
  }



  async function handleOpenSocialKit(candidate: Candidate) {
    setSocialKitModalCandidate(candidate);
    if (!socialKitData[candidate.id]) {
      setSocialKitLoading(candidate.id);
      try {
        const kit = await invoke<SocialKit>("generate_social_kit_for_candidate", { candidateId: candidate.id });
        setSocialKitData((prev) => ({ ...prev, [candidate.id]: kit }));
      } catch (err) {
        console.error("Failed to generate social kit:", err);
      } finally {
        setSocialKitLoading(null);
      }
    }
  }

  async function handleRegenerateSocialKit(candidateId: string) {
    setSocialKitLoading(candidateId);
    try {
      const kit = await invoke<SocialKit>("generate_social_kit_for_candidate", { candidateId });
      setSocialKitData((prev) => ({ ...prev, [candidateId]: kit }));
      showToast("Social kit regenerated");
    } catch (err) {
      console.error("Failed to regenerate social kit:", err);
    } finally {
      setSocialKitLoading(null);
    }
  }

  // Filtered projects
  const filteredProjects = useMemo(() => {
    if (!projectSearch.trim()) return projects;
    const query = projectSearch.toLowerCase();
    return projects.filter((p) => {
      const name = p.name || fileName(p.sourcePath);
      return name.toLowerCase().includes(query) || p.sourcePath.toLowerCase().includes(query);
    });
  }, [projects, projectSearch]);

  // Filtered transcript
  const filteredSegments = useMemo(() => {
    if (!transcript?.segments) return [];
    if (!transcriptSearch.trim()) return transcript.segments;
    const query = transcriptSearch.toLowerCase();
    return transcript.segments.filter((s) => s.text.toLowerCase().includes(query));
  }, [transcript, transcriptSearch]);

  // Filtered candidates
  const filteredCandidates = useMemo(() => {
    if (!detail?.candidates) return [];
    if (momentTab === "selected") return detail.candidates.filter((c) => c.selected);
    if (momentTab === "ready") {
      return detail.candidates.filter((c) => {
        const clip = clipByCandidate.get(c.id);
        return clip?.status === "done" && Boolean(clip.outputPath);
      });
    }
    return detail.candidates;
  }, [detail?.candidates, momentTab, clipByCandidate]);

  if (isOnboarded === null) {
    return (
      <div className="center-loader-screen">
        <Loader2 className="spin" size={32} />
      </div>
    );
  }

  if (isOnboarded === false) {
    return (
      <Onboarding
        environment={environment}
        onComplete={() => setIsOnboarded(true)}
        setTranscriptionEngine={setTranscriptionEngine}
        setLlmEngine={setLlmEngine}
        setLocalLlmModel={setLocalLlmModel}
        setDeepgramKey={setDeepgramKey}
        setGeminiKey={setGeminiKey}
        setAnthropicKey={setAnthropicKey}
        setDeepseekKey={setDeepseekKey}
        setGroqKey={setGroqKey}
        deepgramKey={deepgramKey}
        geminiKey={geminiKey}
        anthropicKey={anthropicKey}
        deepseekKey={deepseekKey}
        groqKey={groqKey}
        refreshEnv={() => refresh()}
      />
    );
  }

  return (
    <div className="app-shell-container">
      <Toast message={toast} onClose={() => setToast(null)} />

      {/* Main Layout */}
      <main className="app-shell">
        {/* Left Sidebar */}
        <aside className="sidebar">
          {/* Brand mark */}
          <div className="brand-row" onClick={() => setDetail(null)} title="Go to All Projects">
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
              onClick={() => handleSectionChange("shorts")}
              title="Shorts & Reels Studio: Single-speaker 9:16 vertical crop"
            >
              <Sparkles size={14} />
              <span>Shorts & Reels</span>
            </button>
            <button
              className={`sidebar-mode-btn podcast ${appSection === "podcast" ? "active" : ""}`}
              onClick={() => handleSectionChange("podcast")}
              title="Podcast Studio: 2-Person 9:16 split-screen for table recordings"
            >
              <Mic size={14} />
              <span>Podcast (9:16)</span>
              <span className="mode-pill-split">2-Face</span>
            </button>
          </div>

          {/* Quick Actions */}
          <div className="sidebar-actions">
            <button className="sidebar-action-btn primary" onClick={importMedia} disabled={busy !== "idle"}>
              {busy === "import" ? <Loader2 className="spin" size={15} /> : <FileVideo size={15} />}
              {appSection === "podcast" ? "Import Podcast Video" : "Import Recording"}
            </button>
            <button
              className="sidebar-action-btn secondary"
              onClick={() => setYoutubeModalOpen(true)}
              disabled={busy !== "idle" || !environment?.hasYtdlp}
              title={!environment?.hasYtdlp ? "yt-dlp required" : "Import from YouTube"}
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
              className={`project-nav-item ${!detail ? "active" : ""}`}
              onClick={() => setDetail(null)}
            >
              <Layers size={14} />
              <span>All Projects</span>
              <ChevronRight size={13} className="nav-arrow" />
            </button>

            {projects.map((project) => (
              <button
                key={project.id}
                className={`project-nav-item ${detail?.project.id === project.id ? "active" : ""}`}
                onClick={() => void selectProject(project.id)}
              >
                <FileVideo size={14} />
                <span className="truncate">{project.name || fileName(project.sourcePath)}</span>
                <ChevronRight size={13} className="nav-arrow" />
              </button>
            ))}
          </section>

          {/* Sidebar Footer with Settings */}
          <div className="sidebar-footer">
            <button
              className="sidebar-action-btn settings-btn"
              onClick={() => setShowSettings(true)}
              title="Global Studio Settings"
            >
              <Settings size={15} />
              <span>Settings</span>
            </button>
          </div>
        </aside>

        {/* Workspace / Content Area */}
        <section className="workspace">
          {detail ? (
            <div className="project-workspace">
              {/* Studio Topbar */}
              <header className="workspace-topbar">
                <div className="topbar-left">
                  <button className="back-btn" onClick={() => setDetail(null)} title="Back to All Projects">
                    <ArrowLeft size={16} />
                    <span>Projects</span>
                  </button>
                  <span className="breadcrumb-slash">/</span>
                  <div className="topbar-title-group">
                    <h2
                      className="project-editable-title"
                      onClick={() => void renameProject(detail.project.id)}
                      title="Click to rename project"
                    >
                      {detail.project.name || fileName(detail.project.sourcePath)}
                      <Edit3 size={13} className="title-edit-icon" />
                    </h2>
                    <div className="project-meta-pills">
                      <span className="meta-pill">
                        {detail.project.sourceDuration ? formatTime(detail.project.sourceDuration) : "Probing..."}
                      </span>
                      <span className="meta-pill">{detail.project.captionStyle || "Modern Box"}</span>
                      <span className="meta-pill status-pill">{detail.project.status}</span>
                    </div>
                  </div>
                </div>

                <div className="topbar-right">
                  <div className="topbar-mode-toggle">
                    <button
                      className={`topbar-mode-tab ${appSection === "shorts" ? "active" : ""}`}
                      onClick={() => handleSectionChange("shorts")}
                      title="Switch to Shorts & Reels Studio"
                    >
                      <Sparkles size={12} />
                      <span>Shorts & Reels</span>
                    </button>
                    <button
                      className={`topbar-mode-tab podcast ${appSection === "podcast" ? "active" : ""}`}
                      onClick={() => handleSectionChange("podcast")}
                      title="Switch to Podcast 2-Person Split Studio"
                    >
                      <Mic size={12} />
                      <span>Podcast Split</span>
                    </button>
                  </div>

                  <button
                    className="topbar-action-btn"
                    onClick={() => {
                      // 1. If any candidate in this project has already been cut and has an output path, open that folder!
                      const renderedClip = detail?.candidates.map((c) => clipByCandidate.get(c.id)).find((cl) => cl?.outputPath);
                      if (renderedClip?.outputPath) {
                        const parts = renderedClip.outputPath.split(/[\/]/);
                        parts.pop(); // remove clip filename (e.g. clip-01_flat.mp4)
                        const folder = parts.join("/");
                        void openFolder(folder);
                        return;
                      }

                      // 2. Otherwise calculate the project clip folder path
                      if (detail?.project) {
                        const rawName = detail.project.name || fileName(detail.project.sourcePath);
                        const slug = rawName
                          .replace(/\.[^/.]+$/, "")
                          .replace(/[^a-zA-Z0-9_-]/g, "-")
                          .replace(/-+/g, "-")
                          .replace(/^-|-$/g, "");
                        const base = clipsSaveDir || defaultFolders?.clipsOutputDir || "";
                        if (base) {
                          void openFolder(`${base}/${slug}/clips`);
                          return;
                        }
                      }

                      // 3. Fallback to base clips directory
                      void openFolder(clipsSaveDir || defaultFolders?.clipsOutputDir || "");
                    }}
                    title="Open Project Clips Folder in Finder"
                  >
                    <FolderOpen size={15} />
                    <span>Clips Folder</span>
                  </button>
                  <button
                    className="topbar-action-btn"
                    onClick={() => setShowSettings(true)}
                    title="Studio Settings"
                  >
                    <Sliders size={15} />
                    <span>Config</span>
                  </button>
                  <button
                    className="topbar-icon-btn"
                    onClick={() => void refresh(detail.project.id)}
                    title="Refresh Studio State"
                  >
                    <RefreshCw size={15} />
                  </button>
                </div>
              </header>

              {/* Error Banner */}
              {error && (
                <div className="workspace-error-banner">
                  <AlertTriangle size={16} />
                  <span>{error}</span>
                  <button onClick={() => setError(null)}><X size={14} /></button>
                </div>
              )}

              {/* 4-Stage Studio Pipeline Tracker */}
              <div className="pipeline-tracker">
                <div className={`pipeline-step ${detail.project.sourcePath ? "complete" : ""}`}>
                  <div className="step-circle">{detail.project.sourcePath ? <Check size={12} /> : "1"}</div>
                  <div className="step-content">
                    <span className="step-title">Source Loaded</span>
                    <span className="step-sub">{fileName(detail.project.sourcePath)}</span>
                  </div>
                </div>
                <div className="pipeline-connector" />

                <div className={`pipeline-step ${Boolean(detail.transcript) ? "complete" : ""}`}>
                  <div className="step-circle">{detail.transcript ? <Check size={12} /> : "2"}</div>
                  <div className="step-content">
                    <span className="step-title">Transcription</span>
                    <span className="step-sub">{transcript ? `${transcript.segments.length} segments` : "Pending"}</span>
                  </div>
                </div>
                <div className="pipeline-connector" />

                <div className={`pipeline-step ${detail.candidates.length > 0 ? "complete" : ""}`}>
                  <div className="step-circle">{detail.candidates.length > 0 ? <Check size={12} /> : "3"}</div>
                  <div className="step-content">
                    <span className="step-title">Viral Moments</span>
                    <span className="step-sub">{detail.candidates.length ? `${detail.candidates.length} found` : "Pending"}</span>
                  </div>
                </div>
                <div className="pipeline-connector" />

                <div className={`pipeline-step ${cutCount > 0 ? "complete" : ""}`}>
                  <div className="step-circle">{cutCount > 0 ? <Check size={12} /> : "4"}</div>
                  <div className="step-content">
                    <span className="step-title">Rendered Clips</span>
                    <span className="step-sub">{cutCount > 0 ? `${cutCount} ready` : "Not cut"}</span>
                  </div>
                </div>
              </div>

              {/* Split-Screen Studio Grid */}
              <div className="studio-grid">
                {/* Left Panel: Transcript Studio */}
                <section className="studio-panel transcript-studio">
                  <div className="panel-header">
                    <div>
                      <h3>Interactive Transcript</h3>
                      <p>{transcript ? `${transcript.segments.length} dialogue segments` : "Audio not transcribed yet"}</p>
                    </div>

                    <button
                      className="studio-btn primary"
                      onClick={transcribe}
                      disabled={busy !== "idle" || !canTranscribe}
                    >
                      {busy === "transcribe" ? <Loader2 className="spin" size={14} /> : <AudioLines size={14} />}
                      {transcript ? "Re-transcribe" : "Transcribe"}
                    </button>
                  </div>

                  {transcript && (
                    <div className="panel-search-bar">
                      <Search size={14} className="search-icon" />
                      <input
                        type="text"
                        placeholder="Search transcript text..."
                        value={transcriptSearch}
                        onChange={(e) => setTranscriptSearch(e.target.value)}
                      />
                      {transcriptSearch && (
                        <button onClick={() => setTranscriptSearch("")} className="clear-search"><X size={13} /></button>
                      )}
                    </div>
                  )}

                  <div className="transcript-scroll-area">
                    {filteredSegments.length > 0 ? (
                      filteredSegments.map((seg, idx) => (
                        <div key={`${seg.start}-${idx}`} className="transcript-segment-card">
                          <div className="segment-meta">
                            <span className="segment-time">{formatTime(seg.start)}</span>
                            {seg.speaker && <span className="segment-speaker">{seg.speaker}</span>}
                          </div>
                          <p className="segment-text">{seg.text}</p>
                        </div>
                      ))
                    ) : transcript ? (
                      <div className="empty-panel-state">
                        <Search size={28} />
                        <p>No matching transcript lines found</p>
                      </div>
                    ) : (
                      <div className="empty-panel-state">
                        <AudioLines size={36} />
                        <h4>No Transcript Available</h4>
                        <p>Run transcription to enable AI moment detection and automated captions.</p>
                        <button
                          className="studio-btn primary"
                          onClick={transcribe}
                          disabled={busy !== "idle" || !canTranscribe}
                          style={{ marginTop: "12px" }}
                        >
                          {busy === "transcribe" ? <Loader2 className="spin" size={14} /> : <AudioLines size={14} />}
                          Transcribe Video ({transcriptionEngine === "local" ? "Whisper Offline" : "Deepgram Cloud"})
                        </button>
                      </div>
                    )}
                  </div>
                </section>

                {/* Right Panel: Viral Moments Studio */}
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
                            Adaptive multi-person timeline reframing: 1 Person (Full 9:16), 2 People (Top/Bottom Split), 3 People (2 Top + 1 Bottom). Dynamically tracks persistent identities, preserves framing during temporary absences, and keeps captions synchronized along dividing seams.
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
                      <p>{detail.candidates.length ? `${selectedCount} selected of ${detail.candidates.length}` : "Run AI detection to find viral hooks"}</p>
                    </div>

                    <div className="panel-header-actions">
                      <button
                        className="studio-btn secondary"
                        onClick={moments}
                        disabled={busy !== "idle" || !detail.transcript || !canUseActiveLlm}
                      >
                        {busy === "moments" ? <Loader2 className="spin" size={14} /> : <Sparkles size={14} />}
                        Find Moments
                      </button>

                      <button
                        className="studio-btn primary"
                        onClick={cutSelected}
                        disabled={busy !== "idle" || selectedCount === 0 || !environment?.hasFfmpeg}
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
                        <button className="batch-btn" onClick={() => selectBatch("top3")}>Top 3</button>
                        <button className="batch-btn" onClick={() => selectBatch("top5")}>Top 5</button>
                        <button className="batch-btn" onClick={() => selectBatch("all")}>All</button>
                        <button className="batch-btn" onClick={() => selectBatch("none")}>Clear</button>
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

                    {/* Punch Zoom, Dead Air Cut, and Studio Audio Buttons: Single Horizontal Line, Side by Side, Equal Spacing & Consistent Sizing */}
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
                  </div>

                  {/* Candidates Cards List */}
                  <div className="candidates-scroll-area">

                    {filteredCandidates.length > 0 ? (
                      filteredCandidates.map((candidate) => {
                        const clip = clipByCandidate.get(candidate.id);
                        const isCut = clip?.status === "done" && Boolean(clip.outputPath);
                        const isCuttingThis = renderingCandidateId === candidate.id;
                        const igPost = instagramPostByCandidate.get(candidate.id);
                        const hasIgPost = Boolean(igPost);
                        const isPublishingThis = publishingCandidateId === candidate.id || igPost?.status === "publishing";

                        return (
                          <article
                            key={candidate.id}
                            className={`moment-candidate-card ${candidate.selected ? "selected" : ""}`}
                          >
                            <div className="moment-card-header">
                              <div className="moment-card-header-left">
                                <button
                                  className="checkbox-toggle-btn"
                                  onClick={() => toggleCandidate(candidate.id)}
                                  title="Toggle clip selection"
                                >
                                  {candidate.selected ? <CheckSquare size={17} className="checked" /> : <Square size={17} />}
                                </button>
                                <span className="moment-rank-badge">#{candidate.rank}</span>
                                <span className="moment-score-badge">
                                  {Math.round(candidate.score > 1 ? candidate.score : candidate.score * 100)}% Viral Score
                                </span>
                                {candidate.rationale.includes("High Audio Energy") && (
                                  <span className="moment-audio-energy-badge" title="High-Energy Audio Hook & Vocal Surge">
                                    ⚡ High Audio Energy
                                  </span>
                                )}
                                {reframeMode === "podcast_split" && (
                                  <span className="candidate-podcast-pill" title="9:16 Two-Person Table Split Screen">
                                    <Users size={11} /> 2-Person Split
                                  </span>
                                )}
                                <span className="moment-duration-badge">
                                  {formatTime(candidate.startSec)} - {formatTime(candidate.endSec)} ({Math.round(candidate.endSec - candidate.startSec)}s)
                                </span>
                              </div>

                              <div className="moment-card-header-right">
                                {(() => {
                                  const igPost = instagramPostByCandidate.get(candidate.id);
                                  if (igPost?.status === "published") {
                                    return (
                                      <a
                                        href={igPost.postUrl || "#"}
                                        target="_blank"
                                        rel="noreferrer"
                                        className="instagram-status-pill published"
                                        onClick={(e) => {
                                          if (igPost.postUrl) {
                                            e.preventDefault();
                                            openFolder(igPost.postUrl);
                                          }
                                        }}
                                        title="View live Instagram Reel"
                                      >
                                        <Instagram size={11} />
                                        <span>Reel Published ↗</span>
                                      </a>
                                    );
                                  }
                                  if (igPost?.status === "publishing" || publishingCandidateId === candidate.id) {
                                    return (
                                      <span className="instagram-status-pill publishing">
                                        <Loader2 className="spin" size={11} />
                                        <span>Posting to IG...</span>
                                      </span>
                                    );
                                  }
                                  if (igPost?.status === "failed") {
                                    return (
                                      <span className="instagram-status-pill failed" title={igPost.errorMessage || "Failed"}>
                                        <AlertTriangle size={11} />
                                        <span>IG Failed</span>
                                      </span>
                                    );
                                  }
                                  return null;
                                })()}
                                <span className={`moment-render-status ${isCut ? "ready" : clip?.status === "error" ? "error" : "pending"}`}>
                                  {isCuttingThis ? "Rendering..." : isCut ? "Ready" : clip?.status === "error" ? "Failed" : "Pending"}
                                </span>
                              </div>
                            </div>

                            <div className="moment-card-body">
                              <h4 className="moment-hook">{candidate.hook}</h4>
                              <p className="moment-rationale">{candidate.rationale}</p>

                              {clip?.outputPath && (
                                <div className="rendered-clip-path">
                                  <span className="path-label">Export:</span>
                                  <span className="path-value truncate">{clip.outputPath}</span>
                                </div>
                              )}
                            </div>


                            <div className="moment-card-actions">
                              <div className="card-actions-left">
                                <button
                                  className="action-pill-btn social-kit"
                                  onClick={() => void handleOpenSocialKit(candidate)}
                                  title="Generate viral titles, hashtags & captions for this clip"
                                >
                                  <Sparkles size={13} />
                                  <span>AI Social Kit</span>
                                </button>

                                <button
                                  className="action-pill-btn cut-action"
                                  onClick={() => void cutCandidate(candidate.id)}
                                  disabled={busy !== "idle" || !environment?.hasFfmpeg}
                                  title={isCut ? "Re-cut this 9:16 vertical clip" : "Cut 9:16 vertical clip with stylized captions"}
                                >
                                  {isCuttingThis ? <Loader2 className="spin" size={13} /> : <Scissors size={13} />}
                                  <span>{isCuttingThis ? "Cutting..." : isCut ? "Re-cut" : "Cut Clip"}</span>
                                </button>

                                {isCut && clip?.outputPath && (
                                  <button
                                    className="action-pill-btn finder"
                                    onClick={() => openFolder(clip.outputPath!)}
                                    title="Reveal clip in macOS Finder"
                                  >
                                    <FolderOpen size={13} />
                                    <span>Finder</span>
                                  </button>
                                )}
                              </div>

                              <div className="card-actions-right">
                                <button
                                  className={`action-pill-btn instagram-publish-btn ${isPublishingThis ? "loading" : ""}`}
                                  onClick={() => void handlePublishToInstagram(candidate.id)}
                                  disabled={isPublishingThis}
                                  title={
                                    isPublishingThis
                                      ? "Publishing clip to Instagram Reels..."
                                      : igPost?.status === "published"
                                      ? "Re-post this clip to Instagram Reels"
                                      : "Automatically cut clip, generate AI caption/hashtags, and post to Instagram Reels"
                                  }
                                >
                                  {isPublishingThis ? (
                                    <Loader2 className="spin" size={13} />
                                  ) : (
                                    <Instagram size={13} />
                                  )}
                                  <span>
                                    {isPublishingThis
                                      ? isCut
                                        ? "Posting..."
                                        : "Cutting & Posting..."
                                      : igPost?.status === "published"
                                      ? "Re-post IG"
                                      : "Post to Reels"}
                                  </span>
                                </button>
                              </div>
                            </div>
                          </article>
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
                            onClick={moments}
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
              </div>
            </div>
          ) : (
            /* All Projects Dashboard (Minimalist & Monochrome) */
            <div className="home-dashboard">
              <header className="home-header">
                <div className="home-header-info">
                  <h2>{appSection === "podcast" ? "Podcast Studio (Dynamic 9:16)" : "All Projects"}</h2>
                  <p>
                    {appSection === "podcast"
                      ? "Transform 16:9 podcasts into dynamic multi-person vertical clips (1, 2, or 3 people) for Instagram Reels."
                      : "Select a project below or import a new media file to get started."}
                  </p>
                </div>
                <div className="home-header-actions">
                  <button className="btn-minimal-primary" onClick={importMedia} disabled={busy !== "idle"}>
                    {busy === "import" ? <Loader2 className="spin" size={15} /> : <FileVideo size={15} />}
                    {appSection === "podcast" ? "Import Podcast Video" : "Import Recording"}
                  </button>
                  <button
                    className="btn-minimal-secondary"
                    onClick={() => setYoutubeModalOpen(true)}
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
                    <button onClick={() => setProjectSearch("")} className="clear-search"><X size={13} /></button>
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
                        <h3 className="project-card-title truncate" title={name}>{name}</h3>
                        <div className="project-card-meta">
                          <span>{project.sourceDuration ? formatTime(project.sourceDuration) : "Probing..."}</span>
                          <span>{formatDate(project.createdAt)}</span>
                        </div>
                        <div className="project-card-actions">
                          <button className="action-btn open-btn" onClick={() => void selectProject(project.id)}>
                            Open Studio
                          </button>
                          <button className="action-btn rename-btn" onClick={() => void renameProject(project.id)}>
                            Rename
                          </button>
                          <button className="action-btn delete-btn" onClick={() => void deleteProject(project.id)}>
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
          )}
        </section>
      </main>

      {/* Modern Status Bar */}
      <footer className="status-bar">
        <div className="status-bar-left">
          <span className="system-dot" />
          <span className="system-text">System Ready</span>
          {environment?.hasHardwareAccel && (
            <span className="accel-pill" title="Apple Silicon VideoToolbox Hardware Acceleration">
              <Zap size={11} /> VideoToolbox GPU Active
            </span>
          )}
        </div>

        <div className="status-bar-right">
          <span className={`status-tag ${environment?.hasFfmpeg ? "active" : ""}`}>ffmpeg</span>
          <span className={`status-tag ${environment?.hasFfprobe ? "active" : ""}`}>ffprobe</span>
          <span className={`status-tag ${environment?.hasYtdlp ? "active" : ""}`}>yt-dlp</span>
          <span className={`status-tag ${environment?.hasLocalWhisperModel ? "active" : ""}`}>Whisper</span>
          <span className={`status-tag ${environment?.hasOllama ? "active" : ""}`}>Ollama</span>
          <span className={`status-tag ${canUseCloudKey ? "active" : ""}`}>Deepgram</span>
        </div>
      </footer>

      {/* Unified Settings Modal */}
      {showSettings && (
        <div className="modal-overlay" onClick={() => setShowSettings(false)}>
          <div className="settings-modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <div className="modal-header-left">
                <div className="modal-icon-badge">
                  <Settings size={18} />
                </div>
                <div>
                  <h3>Studio Settings</h3>
                  <p>Configure AI engines, storage directories, and video exports</p>
                </div>
              </div>
              <button className="modal-close-btn" onClick={() => setShowSettings(false)}><X size={16} /></button>
            </div>

            {/* Tab navigation */}
            <div className="settings-tab-bar">
              <button
                className={`settings-tab-btn ${settingsTab === "ai" ? "active" : ""}`}
                onClick={() => setSettingsTab("ai")}
              >
                <Cpu size={14} /> AI & API Keys
              </button>
              <button
                className={`settings-tab-btn ${settingsTab === "storage" ? "active" : ""}`}
                onClick={() => setSettingsTab("storage")}
              >
                <FolderOpen size={14} /> Storage & Folders
              </button>
              <button
                className={`settings-tab-btn ${settingsTab === "export" ? "active" : ""}`}
                onClick={() => setSettingsTab("export")}
              >
                <SlidersHorizontal size={14} /> Video & Captions
              </button>
              <button
                className={`settings-tab-btn ${settingsTab === "system" ? "active" : ""}`}
                onClick={() => setSettingsTab("system")}
              >
                <Activity size={14} /> System Diagnostics
              </button>
            </div>

            {/* Tab Content */}
            <div className="settings-tab-content">
              {settingsTab === "ai" && (
                <div className="settings-form-stack">
                  <div className="settings-field-group">
                    <label>Transcription Provider</label>
                    <select
                      value={transcriptionEngine}
                      onChange={(e) => setTranscriptionEngine(e.target.value as any)}
                    >
                      <option value="local">Local Whisper (Offline & Free)</option>
                      <option value="deepgram">Deepgram (Cloud API - Super Fast)</option>
                    </select>
                  </div>

                  {transcriptionEngine === "deepgram" && (
                    <div className="settings-field-group">
                      <label>Deepgram API Key</label>
                      <input
                        type="password"
                        value={deepgramKey}
                        onChange={(e) => setDeepgramKey(e.target.value)}
                        placeholder={environment?.hasDeepgramKey ? "Loaded from .env" : "Enter Deepgram API Key"}
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
                        placeholder={environment?.hasAnthropicKey ? "Loaded from .env" : "Enter Anthropic API Key"}
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
                          placeholder={environment?.hasDeepseekKey ? "Loaded from .env" : "Enter DeepSeek API Key"}
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
                        placeholder={environment?.hasGeminiKey ? "Loaded from .env" : "Enter Gemini API Key"}
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
                        placeholder={environment?.hasOpenaiKey ? "Loaded from .env" : "Enter OpenAI API Key"}
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
                        placeholder={environment?.hasGroqKey ? "Loaded from .env" : "Enter Groq API Key"}
                      />
                    </div>
                  )}

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
                      <option value="graph_api">Official Meta Graph API (Direct Instagram Reels)</option>
                      <option value="webhook">Webhook Automation (Make.com, Zapier, n8n)</option>
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
                        <span className="folder-hint">Found in Meta Business Suite or via Graph API Explorer</span>
                      </div>

                      <div className="settings-field-group">
                        <label>Meta Long-Lived Access Token</label>
                        <input
                          type="password"
                          value={instagramAccessToken}
                          onChange={(e) => setInstagramAccessToken(e.target.value)}
                          placeholder="EAA... (Token with instagram_content_publish permission)"
                        />
                        <span className="folder-hint">Requires 'instagram_basic' and 'instagram_content_publish' scopes</span>
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
                      <span className="folder-hint">Payload includes candidateId, videoPath, viralScore, hook, and formatted caption</span>
                    </div>
                  )}

                  <div style={{ marginTop: "4px", display: "flex", alignItems: "center", gap: "12px" }}>
                    <button
                      type="button"
                      className="studio-btn secondary"
                      onClick={testInstagramConnection}
                      disabled={instagramTesting || (instagramProvider === "graph_api" ? !instagramAccountId.trim() || !instagramAccessToken.trim() : !instagramWebhookUrl.trim())}
                    >
                      {instagramTesting ? <Loader2 className="spin" size={14} /> : <Instagram size={14} />}
                      <span>{instagramTesting ? "Verifying..." : "Test Connection"}</span>
                    </button>
                  </div>

                  {instagramTestResult && (
                    <div className={`connection-status-banner ${instagramTestResult.success ? "success" : "error"}`}>
                      {instagramTestResult.success ? <BadgeCheck size={16} /> : <AlertTriangle size={16} />}
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
                      <span className="folder-hint">Where downloaded YouTube videos will be stored</span>
                    </div>
                    <div className="folder-input-row">
                      <input
                        type="text"
                        value={youtubeSaveDir}
                        onChange={(e) => setYoutubeSaveDir(e.target.value)}
                        placeholder={defaultFolders?.youtubeSaveDir || "~/Downloads/ClipOn"}
                      />
                      <button
                        className="studio-btn secondary"
                        onClick={() => browseFolder(youtubeSaveDir || defaultFolders?.youtubeSaveDir || "", setYoutubeSaveDir, "clipon_youtube_dir")}
                        title="Pick folder visually"
                      >
                        Browse...
                      </button>
                      <button
                        className="studio-btn secondary icon-only"
                        onClick={() => openFolder(youtubeSaveDir || defaultFolders?.youtubeSaveDir || "")}
                        title="Open folder in Finder"
                      >
                        <FolderOpen size={15} />
                      </button>
                    </div>
                  </div>

                  {/* Clips Output Folder */}
                  <div className="settings-folder-group">
                    <div className="folder-group-header">
                      <label>Rendered Clips Output Destination</label>
                      <span className="folder-hint">Where final vertical video clips and captions will be saved</span>
                    </div>
                    <div className="folder-input-row">
                      <input
                        type="text"
                        value={clipsSaveDir}
                        onChange={(e) => setClipsSaveDir(e.target.value)}
                        placeholder={defaultFolders?.clipsOutputDir || "~/Documents/ClipOn"}
                      />
                      <button
                        className="studio-btn secondary"
                        onClick={() => browseFolder(clipsSaveDir || defaultFolders?.clipsOutputDir || "", setClipsSaveDir, "clipon_clips_dir")}
                        title="Pick folder visually"
                      >
                        Browse...
                      </button>
                      <button
                        className="studio-btn secondary icon-only"
                        onClick={() => openFolder(clipsSaveDir || defaultFolders?.clipsOutputDir || "")}
                        title="Open folder in Finder"
                      >
                        <FolderOpen size={15} />
                      </button>
                    </div>
                  </div>

                  {/* Storage Cleanup */}
                  <div className="settings-folder-group" style={{ borderColor: "rgba(239, 68, 68, 0.25)", background: "rgba(239, 68, 68, 0.03)" }}>
                    <div className="folder-group-header">
                      <label style={{ color: "#f87171" }}>🗑️ Project Storage Cleanup</label>
                      <span className="folder-hint">Delete all rendered vertical clips, downloaded source videos, and clear database history to free up disk space</span>
                    </div>
                    <div style={{ marginTop: "4px" }}>
                      <button
                        type="button"
                        className="studio-btn secondary"
                        style={{ color: "#f87171", borderColor: "rgba(239, 68, 68, 0.4)" }}
                        onClick={async () => {
                          if (confirm("Are you sure you want to clear all project storage? All downloaded videos, rendered clips, and project records will be wiped.")) {
                            try {
                              const res = await invoke<string>("clear_all_storage");
                              showToast(res);
                              setProjects([]);
                              setDetail(null);
                            } catch (e) {
                              showToast("Failed to clear storage: " + String(e));
                            }
                          }
                        }}
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
                      <option value="vertical_crop">1. Center Crop (Standard 9:16)</option>
                      <option value="podcast_split">2. Podcast Studio (Dynamic 1P / 2P / 3P Split 9:16)</option>
                      <option value="original">3. Original Aspect Ratio</option>
                    </select>
                  </div>

                  <div className="settings-field-group">
                    <label>Retention Punch Zoom</label>
                    <div style={{ display: "flex", alignItems: "center", gap: 10, marginTop: 4 }}>
                      <input
                        type="checkbox"
                        id="setting_punch_zoom"
                        checked={punchZoom}
                        onChange={(e) => {
                          setPunchZoom(e.target.checked);
                          localStorage.setItem("clipon_punch_zoom", String(e.target.checked));
                        }}
                        style={{ width: 16, height: 16, accentColor: "#a855f7", cursor: "pointer" }}
                      />
                      <label htmlFor="setting_punch_zoom" style={{ margin: 0, cursor: "pointer", fontSize: 13, color: "var(--text-secondary)" }}>
                        Retention Punch Zoom Cuts (Punches 1.14x visual zoom every 5.5s to maintain viewer attention across Center Crop, Podcast Split Screen, or Original)
                      </label>
                    </div>
                  </div>

                  <div className="settings-field-group">
                    <label>Dead-Air Silence Jump Cutter</label>
                    <div style={{ display: "flex", alignItems: "center", gap: 10, marginTop: 4 }}>
                      <input
                        type="checkbox"
                        id="setting_remove_silence"
                        checked={removeSilence}
                        onChange={(e) => {
                          setRemoveSilence(e.target.checked);
                          localStorage.setItem("clipon_remove_silence", String(e.target.checked));
                        }}
                        style={{ width: 16, height: 16, accentColor: "#10b981", cursor: "pointer" }}
                      />
                      <label htmlFor="setting_remove_silence" style={{ margin: 0, cursor: "pointer", fontSize: 13, color: "var(--text-secondary)" }}>
                        Automatically skip pauses &amp; dead air &gt;0.45s (Boosts video retention by 20%)
                      </label>
                    </div>
                  </div>

                  <div className="settings-field-group">
                    <label>Studio Sound Mastering</label>
                    <div style={{ display: "flex", alignItems: "center", gap: 10, marginTop: 4 }}>
                      <input
                        type="checkbox"
                        id="setting_studio_audio"
                        checked={studioAudio}
                        onChange={(e) => {
                          setStudioAudio(e.target.checked);
                          localStorage.setItem("clipon_studio_audio", String(e.target.checked));
                        }}
                        style={{ width: 16, height: 16, accentColor: "#facc15", cursor: "pointer" }}
                      />
                      <label htmlFor="setting_studio_audio" style={{ margin: 0, cursor: "pointer", fontSize: 13, color: "var(--text-secondary)" }}>
                        Auto-Master Audio to -14 LUFS Broadcast Standard with AI Spectral Noise Suppression
                      </label>
                    </div>
                  </div>
                </div>
              )}



              {settingsTab === "system" && (
                <div className="settings-form-stack">
                  <div className="diagnostics-list">
                    <div className="diag-item">
                      <span className="diag-name">Apple Silicon VideoToolbox Hardware Accel</span>
                      <span className={`diag-badge ${environment?.hasHardwareAccel ? "ok" : "muted"}`}>
                        {environment?.hasHardwareAccel ? "Active (Hardware Accelerated)" : "Inactive (CPU)"}
                      </span>
                    </div>
                    <div className="diag-item">
                      <span className="diag-name">FFmpeg</span>
                      <span className={`diag-badge ${environment?.hasFfmpeg ? "ok" : "err"}`}>
                        {environment?.hasFfmpeg ? "Installed" : "Missing"}
                      </span>
                    </div>
                    <div className="diag-item">
                      <span className="diag-name">FFprobe</span>
                      <span className={`diag-badge ${environment?.hasFfprobe ? "ok" : "err"}`}>
                        {environment?.hasFfprobe ? "Installed" : "Missing"}
                      </span>
                    </div>
                    <div className="diag-item">
                      <span className="diag-name">yt-dlp (YouTube downloader)</span>
                      <span className={`diag-badge ${environment?.hasYtdlp ? "ok" : "err"}`}>
                        {environment?.hasYtdlp ? "Installed" : "Missing"}
                      </span>
                    </div>
                    <div className="diag-item">
                      <span className="diag-name">Local Whisper (openai-whisper)</span>
                      <span className={`diag-badge ${environment?.hasLocalWhisperModel ? "ok" : "err"}`}>
                        {environment?.hasLocalWhisperModel ? "Installed" : "Missing"}
                      </span>
                    </div>
                    <div className="diag-item">
                      <span className="diag-name">Ollama Local Daemon</span>
                      <span className={`diag-badge ${environment?.hasOllama ? "ok" : "err"}`}>
                        {environment?.hasOllama ? "Running (127.0.0.1:11434)" : "Not detected"}
                      </span>
                    </div>
                  </div>

                  <div className="danger-zone">
                    <label>Danger Zone</label>
                    <p>Reset all configuration and restart onboarding from scratch.</p>
                    <button
                      className="studio-btn danger"
                      onClick={() => {
                        if (window.confirm("Reset all settings and restart onboarding?")) {
                          localStorage.clear();
                          window.location.reload();
                        }
                      }}
                    >
                      Reset Configuration & Onboarding
                    </button>
                  </div>
                </div>
              )}
            </div>

            <div className="modal-footer">
              <button className="studio-btn primary" onClick={() => setShowSettings(false)}>
                Done
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Caption Style Picker Modal */}
      {showStyleModal && (
        <div className="modal-overlay">
          <div className="style-modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <div className="modal-header-left">
                <div className="modal-icon-badge">
                  <Captions size={18} />
                </div>
                <div>
                  <h3>Choose Caption Style</h3>
                  <p>Select automated subtitle typography for your vertical clips</p>
                </div>
              </div>
            </div>

            <div className="style-grid">
              {/* Pro Feature: Hormozi Kinetic Karaoke */}
              <div
                className={`style-card ${selectedStyle === "hormozi-kinetic" ? "selected" : ""}`}
                onClick={() => setSelectedStyle("hormozi-kinetic")}
              >
                <div className="style-preview-box">
                  <span className="preview-text-hormozi" style={{ color: "#00E6FF" }}>KINETIC POP ⚡</span>
                </div>
                <div className="style-card-title">Hormozi Kinetic (Pro Karaoke)</div>
                <div className="style-card-desc">Word-by-word active highlighting with vibrant yellow &amp; electric green keyword pops!</div>
              </div>

              {/* Feature 3: Submagic Viral Style with Auto-Emoji */}
              <div
                className={`style-card ${selectedStyle === "submagic-viral" ? "selected" : ""}`}
                onClick={() => setSelectedStyle("submagic-viral")}
              >
                <div className="style-preview-box">
                  <span className="preview-text-submagic">VIRAL MONEY 💰</span>
                </div>
                <div className="style-card-title">Submagic Viral (Auto-Emoji)</div>
                <div className="style-card-desc">High-retention neon yellow text in a dark pillbox with automated emojis (🔥, 💰, 🤯, 🚀)!</div>
              </div>

              {/* Feature 3: Hormozi Punch Style */}
              <div
                className={`style-card ${selectedStyle === "hormozi-punch" ? "selected" : ""}`}
                onClick={() => setSelectedStyle("hormozi-punch")}
              >
                <div className="style-preview-box">
                  <span className="preview-text-hormozi">CRAZY GAINS 🔥</span>
                </div>
                <div className="style-card-title">Hormozi Punch (Auto-Emoji)</div>
                <div className="style-card-desc">Heavy black text on solid golden yellow box with high-impact auto-emojis.</div>
              </div>

              {/* Feature 3: Neon Cyber Glow */}
              <div
                className={`style-card ${selectedStyle === "neon-glow" ? "selected" : ""}`}
                onClick={() => setSelectedStyle("neon-glow")}
              >
                <div className="style-preview-box">
                  <span className="preview-text-neonglow">TECH SHIFT 🚀</span>
                </div>
                <div className="style-card-title">Neon Glow (Auto-Emoji)</div>
                <div className="style-card-desc">Electric glowing cyan text with deep shadows and contextual auto-emojis.</div>
              </div>

              <div
                className={`style-card ${selectedStyle === "modern-box" ? "selected" : ""}`}
                onClick={() => setSelectedStyle("modern-box")}
              >
                <div className="style-preview-box">
                  <span className="preview-text-box">BRAINFOOD BECAUSE</span>
                </div>
                <div className="style-card-title">Modern Box</div>
                <div className="style-card-desc">Sleek white text inside semi-transparent dark box. Clean & readable.</div>
              </div>

              <div
                className={`style-card ${selectedStyle === "classic-outline" ? "selected" : ""}`}
                onClick={() => setSelectedStyle("classic-outline")}
              >
                <div className="style-preview-box">
                  <span className="preview-text-outline">BRAINFOOD BECAUSE</span>
                </div>
                <div className="style-card-title">Classic Outline</div>
                <div className="style-card-desc">Vibrant bold yellow text with a clean black stroke (CapCut style).</div>
              </div>

              <div
                className={`style-card ${selectedStyle === "minimal-shadow" ? "selected" : ""}`}
                onClick={() => setSelectedStyle("minimal-shadow")}
              >
                <div className="style-preview-box">
                  <span className="preview-text-shadow">BRAINFOOD BECAUSE</span>
                </div>
                <div className="style-card-title">Minimal Shadow</div>
                <div className="style-card-desc">Pure white text with a soft drop shadow. Elegant & unobtrusive.</div>
              </div>

              <div
                className={`style-card ${selectedStyle === "vibrant-cyan" ? "selected" : ""}`}
                onClick={() => setSelectedStyle("vibrant-cyan")}
              >
                <div className="style-preview-box">
                  <span className="preview-text-cyan">BRAINFOOD BECAUSE</span>
                </div>
                <div className="style-card-title">Vibrant Cyan</div>
                <div className="style-card-desc">Vibrant electric cyan text for tech and modern content.</div>
              </div>

              <div
                className={`style-card ${selectedStyle === "vibrant-yellow-box" ? "selected" : ""}`}
                onClick={() => setSelectedStyle("vibrant-yellow-box")}
              >
                <div className="style-preview-box">
                  <span className="preview-text-yellow-box">BRAINFOOD BECAUSE</span>
                </div>
                <div className="style-card-title">Vibrant Yellow Box</div>
                <div className="style-card-desc">Bold dark text inside a solid yellow box. High contrast.</div>
              </div>

              <div
                className={`style-card ${selectedStyle === "vibrant-green" ? "selected" : ""}`}
                onClick={() => setSelectedStyle("vibrant-green")}
              >
                <div className="style-preview-box">
                  <span className="preview-text-green">BRAINFOOD BECAUSE</span>
                </div>
                <div className="style-card-title">Vibrant Green</div>
                <div className="style-card-desc">High-energy neon green with black borders (Hormozi style).</div>
              </div>

              <div
                className={`style-card ${selectedStyle === "vibrant-red" ? "selected" : ""}`}
                onClick={() => setSelectedStyle("vibrant-red")}
              >
                <div className="style-preview-box">
                  <span className="preview-text-red">BRAINFOOD BECAUSE</span>
                </div>
                <div className="style-card-title">Vibrant Red</div>
                <div className="style-card-desc">Dramatic neon crimson for punchy, dramatic hooks.</div>
              </div>
            </div>

            <div className="modal-footer">
              <button
                className="studio-btn secondary"
                onClick={() => { setShowStyleModal(false); setMediaPathToImport(null); }}
              >
                Cancel
              </button>
              <button className="studio-btn primary" onClick={() => confirmImport(selectedStyle)}>
                Confirm & Import
              </button>
            </div>
          </div>
        </div>
      )}

      {/* YouTube Import Modal */}
      {youtubeModalOpen && (
        <div className="modal-overlay" onClick={() => setYoutubeModalOpen(false)}>
          <div className="youtube-modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <div className="modal-header-left">
                <div className="modal-icon-badge">
                  <Youtube size={18} />
                </div>
                <div>
                  <h3>Import from YouTube</h3>
                  <p>Download and convert a YouTube video directly into ClipOn</p>
                </div>
              </div>
              <button className="modal-close-btn" onClick={() => setYoutubeModalOpen(false)}><X size={16} /></button>
            </div>

            <div className="youtube-modal-body">
              <input
                type="text"
                placeholder="https://www.youtube.com/watch?v=..."
                value={youtubeUrl}
                onChange={(e) => setYoutubeUrl(e.target.value)}
                disabled={youtubeStatus !== "idle" && youtubeStatus !== "warning"}
                className="youtube-url-input"
              />

              {youtubeStatus === "warning" && (
                <div className="youtube-warning-box">
                  <div className="warning-title">
                    <AlertTriangle size={18} />
                    <span>Copyright Advisory</span>
                  </div>
                  <p>
                    This video is not explicitly marked with a Creative Commons license. Detected license: <strong>{youtubeWarningLicense}</strong>.
                    Clipping and republishing copyrighted content may violate platform terms.
                  </p>
                </div>
              )}
            </div>

            <div className="modal-footer">
              <button
                className="studio-btn secondary"
                onClick={() => { setYoutubeModalOpen(false); setYoutubeUrl(""); setYoutubeStatus("idle"); }}
                disabled={youtubeStatus === "checking" || youtubeStatus === "downloading"}
              >
                Cancel
              </button>
              {youtubeStatus === "warning" ? (
                <button className="studio-btn danger" onClick={executeYoutubeDownload}>
                  Proceed Anyway
                </button>
              ) : (
                <button
                  className="studio-btn primary"
                  onClick={handleYoutubeImport}
                  disabled={!youtubeUrl || youtubeStatus !== "idle"}
                >
                  {youtubeStatus === "checking" ? <><Loader2 className="spin" size={14} /> Checking...</> :
                   youtubeStatus === "downloading" ? <><Loader2 className="spin" size={14} /> Downloading...</> :
                   "Download & Import"}
                </button>
              )}
            </div>
          </div>
        </div>
      )}

      {/* AI Social Kit Modal */}
      {socialKitModalCandidate && (
        <div className="modal-overlay" onClick={() => setSocialKitModalCandidate(null)}>
          <div className="social-kit-modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <div className="modal-header-left">
                <div className="modal-icon-badge">
                  <Sparkles size={18} />
                </div>
                <div>
                  <h3>AI Social Publishing Kit</h3>
                  <p>Viral titles, hashtags & captions for Clip #{socialKitModalCandidate.rank}</p>
                </div>
              </div>
              <button className="modal-close-btn" onClick={() => setSocialKitModalCandidate(null)}><X size={16} /></button>
            </div>

            <div className="social-kit-modal-body">
              {socialKitLoading === socialKitModalCandidate.id ? (
                <div className="center-loader-box">
                  <Loader2 className="spin" size={28} />
                  <p>Generating high-converting viral titles & hashtags...</p>
                </div>
              ) : socialKitData[socialKitModalCandidate.id] ? (
                (() => {
                  const kit = socialKitData[socialKitModalCandidate.id];
                  const fullPost = `${kit.titles[0]}

${kit.description}

${kit.callToAction}

${kit.hashtags.join(" ")}`;
                  return (
                    <div className="social-kit-content">
                      {/* Viral Titles */}
                      <div className="social-section">
                        <label className="section-label"><Flame size={13} /> High-CTR Titles (Click to copy)</label>
                        <div className="titles-stack">
                          {kit.titles.map((title, i) => (
                            <div
                              key={i}
                              className="copy-item-row"
                              onClick={() => {
                                navigator.clipboard.writeText(title);
                                showToast("Title copied to clipboard!");
                              }}
                            >
                              <span>{title}</span>
                              <Copy size={13} className="copy-icon" />
                            </div>
                          ))}
                        </div>
                      </div>

                      {/* Hashtags */}
                      <div className="social-section">
                        <div className="section-header-row">
                          <label className="section-label"><Hash size={13} /> Trending Hashtags</label>
                          <button
                            className="text-action-btn"
                            onClick={() => {
                              navigator.clipboard.writeText(kit.hashtags.join(" "));
                              showToast("All hashtags copied!");
                            }}
                          >
                            Copy All Tags
                          </button>
                        </div>
                        <div className="hashtag-chips-wrap">
                          {kit.hashtags.map((tag, i) => (
                            <span
                              key={i}
                              className="hashtag-chip"
                              onClick={() => {
                                navigator.clipboard.writeText(tag);
                                showToast(`Copied ${tag}`);
                              }}
                              title="Click to copy"
                            >
                              {tag}
                            </span>
                          ))}
                        </div>
                      </div>

                      {/* Description & Caption */}
                      <div className="social-section">
                        <div className="section-header-row">
                          <label className="section-label">📝 Caption & Description</label>
                          <button
                            className="text-action-btn"
                            onClick={() => {
                              navigator.clipboard.writeText(`${kit.description}

${kit.callToAction}`);
                              showToast("Caption copied!");
                            }}
                          >
                            Copy Caption
                          </button>
                        </div>
                        <div className="caption-preview-box">
                          <p>{kit.description}</p>
                          <span className="caption-cta">{kit.callToAction}</span>
                        </div>
                      </div>

                      {/* Master Copy Button */}
                      <div className="social-master-actions">
                        <button
                          className="studio-btn primary full-width"
                          onClick={() => {
                            navigator.clipboard.writeText(fullPost);
                            showToast("Full Social Post Package Copied!");
                          }}
                        >
                          <Copy size={15} />
                          Copy Complete Social Package (TikTok / Reels / Shorts)
                        </button>
                        <button
                          className="studio-btn secondary icon-only"
                          onClick={() => void handleRegenerateSocialKit(socialKitModalCandidate.id)}
                          title="Regenerate with AI"
                        >
                          <RefreshCw size={14} />
                        </button>
                      </div>
                    </div>
                  );
                })()
              ) : null}
            </div>
          </div>
        </div>
      )}

      {/* Official Meta Graph API Quick Setup Modal */}
      {showMetaModal && (
        <div className="modal-overlay" onClick={() => setShowMetaModal(false)}>
          <div className="settings-modal" style={{ width: "min(560px, 94vw)" }} onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <div className="modal-header-left">
                <div className="modal-icon-badge" style={{ background: "linear-gradient(135deg, rgba(225, 48, 108, 0.2) 0%, rgba(131, 58, 180, 0.2) 100%)", color: "#fb7185", borderColor: "rgba(225, 48, 108, 0.4)" }}>
                  <Instagram size={18} />
                </div>
                <div>
                  <h3>Official Meta Graph API Setup</h3>
                  <p>Post high-viral Reels directly to Instagram with AI captions & hashtags</p>
                </div>
              </div>
              <button className="modal-close-btn" onClick={() => setShowMetaModal(false)}>
                <X size={16} />
              </button>
            </div>

            <div className="modal-body" style={{ padding: "20px" }}>
              <div style={{ background: "rgba(225, 48, 108, 0.08)", border: "1px solid rgba(225, 48, 108, 0.25)", borderRadius: "8px", padding: "12px 14px", marginBottom: "16px", fontSize: "12px", color: "#fbcfe8", lineHeight: "1.5" }}>
                <strong style={{ color: "#ffffff", display: "block", marginBottom: "4px" }}>Meta Graph API Direct Publishing</strong>
                Enter your Instagram Professional Account ID and Meta Graph API Access Token. Once entered, ClipOn will automatically render vertical clips, generate engaging AI titles, captions, and hashtags, and publish directly to your Instagram Reels!
              </div>

              <div className="settings-form-stack">
                <div className="settings-field-group">
                  <label style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                    <span>Instagram Professional / Creator Account ID</span>
                    <button
                      type="button"
                      className="action-pill-btn"
                      style={{ height: "22px", fontSize: "10.5px", padding: "0 6px" }}
                      onClick={() => openFolder("https://developers.facebook.com/tools/explorer/")}
                      title="Open Meta Graph API Explorer in browser"
                    >
                      <ExternalLink size={10} />
                      <span>Graph Explorer</span>
                    </button>
                  </label>
                  <input
                    type="text"
                    value={metaModalAccountId}
                    onChange={(e) => setMetaModalAccountId(e.target.value)}
                    placeholder="e.g. 17841400000000000"
                    autoFocus
                  />
                  <span className="folder-hint">Found in Meta Business Suite or via Graph API Explorer (/me/accounts)</span>
                </div>

                <div className="settings-field-group">
                  <label>Meta User / Page Access Token</label>
                  <input
                    type="password"
                    value={metaModalAccessToken}
                    onChange={(e) => setMetaModalAccessToken(e.target.value)}
                    placeholder="EAA... (Token with instagram_basic and instagram_content_publish permissions)"
                  />
                  <span className="folder-hint">Requires 'instagram_basic' and 'instagram_content_publish' permissions</span>
                </div>

                <div style={{ display: "flex", gap: "8px", marginTop: "4px" }}>
                  <button
                    type="button"
                    className="studio-btn secondary small"
                    onClick={() => void handleTestMetaConnection()}
                    disabled={metaModalTesting}
                  >
                    {metaModalTesting ? <Loader2 className="spin" size={12} /> : <Check size={12} />}
                    <span>{metaModalTesting ? "Testing Connection..." : "Test Connection"}</span>
                  </button>
                </div>

                {metaModalStatus && (
                  <div className={`connection-status-banner ${metaModalStatus.success ? "success" : "error"}`}>
                    {metaModalStatus.success ? <BadgeCheck size={16} /> : <AlertTriangle size={16} />}
                    <span>{metaModalStatus.message}</span>
                  </div>
                )}
              </div>
            </div>

            <div className="modal-footer">
              <button className="studio-btn secondary" onClick={() => setShowMetaModal(false)}>
                Cancel
              </button>
              <button
                className="studio-btn instagram-active"
                onClick={() => void handleSaveMetaAndPost()}
                disabled={metaModalSaving}
              >
                {metaModalSaving ? <Loader2 className="spin" size={14} /> : <Instagram size={14} />}
                <span>{metaModalSaving ? "Saving & Posting..." : pendingCandidateIdToPost ? "Save & Post to Instagram" : "Save Credentials"}</span>
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Ollama Model Pull Progress Modal */}
      {downloadingModelName && (
        <div className="modal-overlay" style={{ zIndex: 10000 }}>
          <div className="download-modal">
            <h3>Downloading Ollama Model</h3>
            <p>Fetching model weights for "{downloadingModelName}". Keep ClipOn open.</p>

            <div className="download-progress-bar">
              <div className="progress-fill" style={{ width: `${modelDownloadProgress}%` }} />
            </div>

            <div className="download-stats-row">
              <span>{modelDownloadStatus}</span>
              <strong>{modelDownloadProgress}%</strong>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ===== Onboarding Component =====
interface OnboardingProps {
  environment: EnvironmentStatus | null;
  onComplete: () => void;
  setTranscriptionEngine: (engine: "deepgram" | "local") => void;
  setLlmEngine: (engine: "claude" | "deepseek" | "local" | "gemini" | "openai" | "openrouter" | "groq") => void;
  setLocalLlmModel: (model: string) => void;
  setDeepgramKey: (key: string) => void;
  setGeminiKey: (key: string) => void;
  setAnthropicKey: (key: string) => void;
  setDeepseekKey: (key: string) => void;
  setGroqKey: (key: string) => void;
  deepgramKey: string;
  geminiKey: string;
  anthropicKey: string;
  deepseekKey: string;
  groqKey: string;
  refreshEnv: () => Promise<void>;
}

function Onboarding({
  environment,
  onComplete,
  setTranscriptionEngine,
  setLlmEngine,
  setLocalLlmModel,
  setDeepgramKey,
  setGeminiKey,
  setAnthropicKey,
  setDeepseekKey,
  setGroqKey,
  deepgramKey: initialDeepgramKey,
  geminiKey: initialGeminiKey,
  anthropicKey: initialAnthropicKey,
  deepseekKey: initialDeepseekKey,
  groqKey: initialGroqKey,
  refreshEnv,
}: OnboardingProps) {
  const [setupMode, setSetupMode] = useState<"choose" | "local" | "cloud" | "downloading">("choose");
  const [selectedModel, setSelectedModel] = useState<string>("llama3.2");
  const [dgKey, setDgKey] = useState(initialDeepgramKey || environment?.deepgramKey || "");
  const [gmKey, setGmKey] = useState(initialGeminiKey || environment?.geminiKey || "");
  const [antKey, setAntKey] = useState(initialAnthropicKey || environment?.anthropicKey || "");
  const [dsKey, setDsKey] = useState(initialDeepseekKey || environment?.deepseekKey || "");
  const [grKey, setGrKey] = useState(initialGroqKey || environment?.groqKey || "");
  const [downloadStatus, setDownloadStatus] = useState("Initializing download...");
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [checkingOllama, setCheckingOllama] = useState(false);
  const [copied, setCopied] = useState(false);

  const skipToStudio = () => {
    localStorage.setItem("clipon_onboarded", "true");
    onComplete();
  };

  useEffect(() => {
    if (!dgKey && (initialDeepgramKey || environment?.deepgramKey)) setDgKey(initialDeepgramKey || environment?.deepgramKey || "");
    if (!gmKey && (initialGeminiKey || environment?.geminiKey)) setGmKey(initialGeminiKey || environment?.geminiKey || "");
    if (!antKey && (initialAnthropicKey || environment?.anthropicKey)) setAntKey(initialAnthropicKey || environment?.anthropicKey || "");
    if (!dsKey && (initialDeepseekKey || environment?.deepseekKey)) setDsKey(initialDeepseekKey || environment?.deepseekKey || "");
    if (!grKey && (initialGroqKey || environment?.groqKey)) setGrKey(initialGroqKey || environment?.groqKey || "");
  }, [environment, initialDeepgramKey, initialGeminiKey, initialAnthropicKey, initialDeepseekKey, initialGroqKey]);

  const copyWhisperCommand = () => {
    navigator.clipboard.writeText("pip3 install -U openai-whisper");
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleCloudSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (dgKey.trim()) {
      setTranscriptionEngine("deepgram");
      setDeepgramKey(dgKey.trim());
      localStorage.setItem("clipon_deepgram_key", dgKey.trim());
      localStorage.setItem("clipon_transcription_engine", "deepgram");
    }

    if (gmKey.trim()) {
      let clean = gmKey.trim();
      if (clean.startsWith("Q.Ab8")) clean = "A" + clean;
      setLlmEngine("gemini");
      setGeminiKey(clean);
      localStorage.setItem("clipon_gemini_key", clean);
      localStorage.setItem("clipon_llm_engine", "gemini");
    } else if (antKey.trim()) {
      setLlmEngine("claude");
      setAnthropicKey(antKey.trim());
      localStorage.setItem("clipon_anthropic_key", antKey.trim());
      localStorage.setItem("clipon_llm_engine", "claude");
    } else if (dsKey.trim()) {
      setLlmEngine("deepseek");
      setDeepseekKey(dsKey.trim());
      localStorage.setItem("clipon_deepseek_key", dsKey.trim());
      localStorage.setItem("clipon_llm_engine", "deepseek");
    } else if (grKey.trim()) {
      setLlmEngine("groq");
      setGroqKey(grKey.trim());
      localStorage.setItem("clipon_groq_key", grKey.trim());
      localStorage.setItem("clipon_llm_engine", "groq");
    }

    localStorage.setItem("clipon_onboarded", "true");
    onComplete();
  };

  const startLocalSetup = async () => {
    setError(null);
    setCheckingOllama(true);
    setDownloadProgress(0);
    await refreshEnv();

    let isOllamaRunning = false;
    try {
      const currentEnv = await invoke<EnvironmentStatus>("environment_status");
      isOllamaRunning = currentEnv.hasOllama;
    } catch (e) {}

    setCheckingOllama(false);

    if (!isOllamaRunning) {
      setSetupMode("downloading");
      setDownloadStatus("Ollama not found. Starting automatic installer...");
      try {
        const unlistenInstall = await listen<string>("ollama-install-status", (event) => {
          setDownloadStatus(event.payload);
        });
        await invoke("install_ollama");
        unlistenInstall();
      } catch (err) {
        setError("Automatic installation failed: " + String(err) + ". Please install it manually from ollama.com.");
        setSetupMode("local");
        return;
      }
    }

    setSetupMode("downloading");
    setDownloadStatus("Ollama connected. Initiating model download...");

    try {
      const unlisten = await listen<{ status: string; completed?: number; total?: number; percentage?: number }>("ollama-pull-progress", (event) => {
        const payload = event.payload;
        setDownloadStatus(payload.status);
        if (payload.percentage !== undefined && payload.percentage !== null) {
          setDownloadProgress(Math.round(payload.percentage));
        }
      });

      await invoke("pull_ollama_model", { modelName: selectedModel });
      unlisten();

      setTranscriptionEngine("local");
      setLlmEngine("local");
      setLocalLlmModel(selectedModel);
      localStorage.setItem("clipon_transcription_engine", "local");
      localStorage.setItem("clipon_llm_engine", "local");
      localStorage.setItem("clipon_local_llm_model", selectedModel);
      localStorage.setItem("clipon_onboarded", "true");
      onComplete();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setSetupMode("local");
    }
  };

  return (
    <div className="modal-overlay">
      <div className="onboarding-card">
        {setupMode === "choose" && (
          <>
            <div className="onboarding-header">
              <div className="brand-mark large">
                <Clapperboard size={36} />
              </div>
              <h2>Welcome to ClipOn</h2>
              <p>Long recording in. Short clips out. Choose how you want to run the studio.</p>
            </div>

            <div className="onboarding-choices">
              <div className="choice-card" onClick={() => setSetupMode("local")}>
                <div className="choice-icon">
                  <Database size={28} />
                </div>
                <h3>Fully Offline & Private</h3>
                <p>Process everything locally on your machine. 100% private, free, and offline.</p>
                <div className="choice-badge local">Offline (Ollama + Whisper)</div>
              </div>

              <div className="choice-card" onClick={() => setSetupMode("cloud")}>
                <div className="choice-icon">
                  <Cloud size={28} />
                </div>
                <h3>Cloud APIs</h3>
                <p>Blazing fast cloud transcription & analysis. Minimal local RAM requirements.</p>
                <div className="choice-badge cloud">API Keys Required</div>
              </div>
            </div>
            <div style={{ marginTop: "24px", display: "flex", justifyContent: "center" }}>
              <button type="button" className="studio-btn secondary" onClick={skipToStudio}>
                Skip Setup & Explore Studio
              </button>
            </div>
          </>
        )}

        {setupMode === "local" && (
          <div className="local-setup-flow">
            <div className="onboarding-header compact">
              <h2>Configure Offline Mode</h2>
              <p>Set up your local transcription and Ollama intelligence.</p>
            </div>

            {error && <div className="workspace-error-banner" style={{ marginBottom: "16px" }}>{error}</div>}

            <div className="setup-steps">
              <div className="setup-step">
                <div className="step-num">1</div>
                <div className="step-body">
                  <h4>Install Python Whisper</h4>
                  <p>Run the following command in terminal to enable local transcription:</p>
                  <div className="code-block-container">
                    <code>pip3 install -U openai-whisper</code>
                    <button type="button" className="copy-btn" onClick={copyWhisperCommand}>
                      {copied ? <Check size={14} /> : <Copy size={14} />}
                      {copied ? "Copied!" : "Copy"}
                    </button>
                  </div>
                  {environment?.hasLocalWhisperModel ? (
                    <span className="step-check success"><BadgeCheck size={14} /> Whisper detected in Python!</span>
                  ) : (
                    <span className="step-check warning">Package 'whisper' not detected yet. Run command above.</span>
                  )}
                </div>
              </div>

              <div className="setup-step">
                <div className="step-num">2</div>
                <div className="step-body">
                  <h4>Select Ollama Local Model</h4>
                  <div className="model-cards">
                    <div
                      className={`model-card ${selectedModel === "llama3.2" ? "active" : ""}`}
                      onClick={() => setSelectedModel("llama3.2")}
                    >
                      <div className="model-card-header">
                        <h5>LLaMA 3.2 3B</h5>
                        <span className="model-size">1.9 GB</span>
                      </div>
                      <p>Fast, efficient, excellent hook identification for shorts.</p>
                    </div>

                    <div
                      className={`model-card ${selectedModel === "qwen2.5:3b" ? "active" : ""}`}
                      onClick={() => setSelectedModel("qwen2.5:3b")}
                    >
                      <div className="model-card-header">
                        <h5>Qwen 2.5 3B</h5>
                        <span className="model-size">2.0 GB</span>
                      </div>
                      <p>Optimized for multilingual dialogues and concise hooks.</p>
                    </div>
                  </div>
                </div>
              </div>
            </div>

            <div className="onboarding-actions">
              <button type="button" className="studio-btn secondary" onClick={() => setSetupMode("choose")}>Back</button>
              <button type="button" className="studio-btn secondary" onClick={skipToStudio}>Skip Setup</button>
              <button
                type="button"
                className="studio-btn primary"
                onClick={startLocalSetup}
                disabled={checkingOllama}
              >
                {checkingOllama ? <Loader2 className="spin" size={16} /> : null}
                {checkingOllama ? "Connecting Ollama..." : "Download & Finish Setup"}
              </button>
            </div>
          </div>
        )}

        {setupMode === "cloud" && (
          <form className="cloud-setup-flow" onSubmit={handleCloudSubmit}>
            <div className="onboarding-header compact">
              <h2>Configure Cloud APIs</h2>
              <p>Add your API keys to enable cloud processing.</p>
            </div>

            {error && <div className="workspace-error-banner" style={{ marginBottom: "16px" }}>{error}</div>}

            <div className="form-stack">
              <div className="settings-field-group">
                <label>Deepgram API Key (Transcription)</label>
                <input
                  type="password"
                  value={dgKey}
                  onChange={(e) => setDgKey(e.target.value)}
                  placeholder={environment?.hasDeepgramKey ? "Loaded from .env" : "Deepgram API Key"}
                />
              </div>

              <div className="settings-field-group">
                <label>Google Gemini API Key (Recommended)</label>
                <input
                  type="password"
                  value={gmKey}
                  onChange={(e) => setGmKey(e.target.value)}
                  placeholder={environment?.hasGeminiKey ? "Loaded from .env" : "Google Gemini API Key"}
                />
              </div>

              <div className="settings-field-group">
                <label>Claude API Key</label>
                <input
                  type="password"
                  value={antKey}
                  onChange={(e) => setAntKey(e.target.value)}
                  placeholder="Anthropic API Key"
                />
              </div>

              <div className="settings-field-group">
                <label>DeepSeek API Key</label>
                <input
                  type="password"
                  value={dsKey}
                  onChange={(e) => setDsKey(e.target.value)}
                  placeholder="DeepSeek API Key"
                />
              </div>

              <div className="settings-field-group">
                <label>Groq API Key</label>
                <input
                  type="password"
                  value={grKey}
                  onChange={(e) => setGrKey(e.target.value)}
                  placeholder="Groq API Key"
                />
              </div>
            </div>

            <div className="onboarding-actions">
              <button type="button" className="studio-btn secondary" onClick={() => setSetupMode("choose")}>Back</button>
              <button type="button" className="studio-btn secondary" onClick={skipToStudio}>Skip for Now</button>
              <button type="submit" className="studio-btn primary">Save & Launch Studio</button>
            </div>
          </form>
        )}

        {setupMode === "downloading" && (
          <div className="downloading-flow">
            <div className="onboarding-header compact">
              <h2>Preparing Local Studio</h2>
              <p>Downloading local model weights. Please keep ClipOn open.</p>
            </div>

            <div className="download-progress-bar">
              <div className="progress-fill" style={{ width: `${downloadProgress}%` }} />
            </div>

            <div className="download-stats-row">
              <span>{downloadStatus}</span>
              <strong>{downloadProgress}%</strong>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
