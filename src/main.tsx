import React, { useEffect, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Loader2, Check } from "lucide-react";
import "./styles.css";

// Feature modules
import { ErrorProvider, useAppError } from "./features/error/ErrorProvider";
import { YoutubeImportModal } from "./features/youtube/YoutubeImportModal";
import { SocialKitModal } from "./features/social/SocialKitModal";
import { InstagramPublishModal } from "./features/social/InstagramPublishModal";
import { SettingsModal } from "./features/settings/SettingsModal";
import { CaptionStyleModal } from "./features/rendering/CaptionStyleModal";
import { Onboarding } from "./features/onboarding/OnboardingModal";
import { ProjectSidebar } from "./features/projects/ProjectSidebar";
import { ProjectHeader } from "./features/projects/ProjectHeader";
import { ProjectsDashboard } from "./features/projects/ProjectsDashboard";
import { TranscriptionPanel } from "./features/transcription/TranscriptionPanel";
import { MomentsPanel } from "./features/moments/MomentsPanel";
import { StatusBar } from "./features/system/StatusBar";

import type {
  EnvironmentStatus,
  Project,
  Transcript,
  SocialKit,
  Candidate,
  Clip,
  InstagramPost,
  ProjectDetail,
  NormalizedTranscript,
  BusyState,
  ReframeMode,
  AppSection,
  ExportPresetPlatform,
} from "./types";

// Utility Helpers
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

function AppContent() {
  const { showError, showWarning, showInfo } = useAppError();
  const [environment, setEnvironment] = useState<EnvironmentStatus | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [detail, setDetail] = useState<ProjectDetail | null>(null);
  const [busy, setBusy] = useState<BusyState>("idle");
  const [toast, setToast] = useState<string | null>(null);

  // Modals state
  const [showSettings, setShowSettings] = useState(false);
  const [showStyleModal, setShowStyleModal] = useState(false);
  const [selectedStyle, setSelectedStyle] = useState("modern-box");
  const [mediaPathToImport, setMediaPathToImport] = useState<string | null>(null);
  const [youtubeModalOpen, setYoutubeModalOpen] = useState(false);

  // Candidate cut & social kit state
  const [renderingCandidateId, setRenderingCandidateId] = useState<string | null>(null);
  const [socialKitModalCandidate, setSocialKitModalCandidate] = useState<Candidate | null>(null);
  const [socialKitData, setSocialKitData] = useState<Record<string, SocialKit>>({});
  const [socialKitLoading, setSocialKitLoading] = useState<string | null>(null);

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
  const [deepgramKey, setDeepgramKey] = useState("");
  const [anthropicKey, setAnthropicKey] = useState("");
  const [deepseekKey, setDeepseekKey] = useState("");
  const [deepseekModel, setDeepseekModel] = useState(() => (localStorage.getItem("clipon_deepseek_model") || localStorage.getItem("autoshorts_deepseek_model")) || "");
  const [geminiKey, setGeminiKey] = useState("");
  const [openaiKey, setOpenaiKey] = useState("");
  const [openrouterKey, setOpenrouterKey] = useState("");
  const [groqKey, setGroqKey] = useState("");

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
    return localStorage.getItem("clipon_reframe_mode") === "punch_zoom";
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

  const [exportPreset, setExportPreset] = useState<ExportPresetPlatform>(() => {
    return (localStorage.getItem("clipon_export_preset") as ExportPresetPlatform) || "instagram_reels";
  });
  useEffect(() => { localStorage.setItem("clipon_export_preset", exportPreset); }, [exportPreset]);

  // Instagram Reels API State
  const [instagramProvider, setInstagramProvider] = useState<"graph_api" | "webhook">(() => {
    return (localStorage.getItem("clipon_instagram_provider") as any) || "graph_api";
  });
  const [instagramAccountId, setInstagramAccountId] = useState(() => {
    return localStorage.getItem("clipon_instagram_account_id") || "";
  });
  const [instagramAccessToken, setInstagramAccessToken] = useState("");
  const [instagramWebhookUrl, setInstagramWebhookUrl] = useState(() => {
    return localStorage.getItem("clipon_instagram_webhook_url") || "";
  });
  const [instagramTesting, setInstagramTesting] = useState(false);
  const [instagramTestResult, setInstagramTestResult] = useState<{ success: boolean; message: string } | null>(null);
  const [publishingCandidateId, setPublishingCandidateId] = useState<string | null>(null);

  // Meta Graph API Quick Connect Modal State
  const [showMetaModal, setShowMetaModal] = useState(false);
  const [pendingCandidateIdToPost, setPendingCandidateIdToPost] = useState<string | null>(null);
  const [metaModalAccountId, setMetaModalAccountId] = useState("");
  const [metaModalAccessToken, setMetaModalAccessToken] = useState("");
  const [metaModalSaving, setMetaModalSaving] = useState(false);
  const [metaModalTesting, setMetaModalTesting] = useState(false);
  const [metaModalStatus, setMetaModalStatus] = useState<{ success: boolean; message: string } | null>(null);

  useEffect(() => { localStorage.setItem("clipon_instagram_provider", instagramProvider); }, [instagramProvider]);
  useEffect(() => { localStorage.setItem("clipon_instagram_account_id", instagramAccountId); }, [instagramAccountId]);
  useEffect(() => { localStorage.setItem("clipon_instagram_webhook_url", instagramWebhookUrl); }, [instagramWebhookUrl]);

  // UI Filters
  const [momentTab, setMomentTab] = useState<"all" | "selected" | "ready">("all");

  const showToast = (msg: string) => setToast(msg);

  // Sync state with LocalStorage
  useEffect(() => { localStorage.setItem("clipon_transcription_engine", transcriptionEngine); }, [transcriptionEngine]);
  useEffect(() => { localStorage.setItem("clipon_llm_engine", llmEngine); }, [llmEngine]);
  useEffect(() => { localStorage.setItem("clipon_local_llm_model", localLlmModel); }, [localLlmModel]);
  useEffect(() => { localStorage.setItem("clipon_deepseek_model", deepseekModel); }, [deepseekModel]);
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

    invoke<{ youtube_download_dir: string; clips_output_dir: string }>("get_default_folders")
      .then((dirs) => setDefaultFolders({ youtubeSaveDir: dirs.youtube_download_dir, clipsOutputDir: dirs.clips_output_dir }))
      .catch(() => {});
  }, []);

  // Global Keyboard Shortcuts (Issue #26)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Escape: close any open modal
      if (e.key === "Escape") {
        setShowSettings(false);
        setYoutubeModalOpen(false);
        setShowStyleModal(false);
        setSocialKitModalCandidate(null);
        setShowMetaModal(false);
        return;
      }

      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;

      // Cmd/Ctrl + , -> Settings
      if (e.key === ",") {
        e.preventDefault();
        setShowSettings((prev) => !prev);
      }
      // Cmd/Ctrl + I -> Import Media
      else if (e.key.toLowerCase() === "i" && !e.shiftKey) {
        e.preventDefault();
        void importMedia();
      }
      // Cmd/Ctrl + Y -> YouTube Import
      else if (e.key.toLowerCase() === "y") {
        e.preventDefault();
        setYoutubeModalOpen(true);
      }
      // Cmd/Ctrl + R -> Refresh Studio State
      else if (e.key.toLowerCase() === "r" && !e.shiftKey) {
        e.preventDefault();
        void refresh();
        showToast("Refreshed studio state");
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  // Refresh data from Rust backend
  async function refresh(nextProjectId?: string) {
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
      if (env.instagramAccountId && (!instagramAccountId || !localStorage.getItem("clipon_instagram_account_id"))) {
        setInstagramAccountId(env.instagramAccountId);
        localStorage.setItem("clipon_instagram_account_id", env.instagramAccountId);
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
    const currentAccId = instagramAccountId.trim() || environment?.instagramAccountId?.trim() || "";
    const hasToken = Boolean(instagramAccessToken.trim() || environment?.hasInstagramToken);

    if (instagramProvider === "graph_api" && (!currentAccId || !hasToken)) {
      setPendingCandidateIdToPost(candidateId);
      setShowMetaModal(true);
      return;
    }
    if (instagramProvider === "webhook" && !instagramWebhookUrl.trim()) {
      setShowSettings(true);
      showWarning("Configure your Instagram Webhook URL in Settings");
      return;
    }

    await executeInstagramPublish(candidateId, currentAccId, instagramAccessToken.trim() || undefined);
  }

  async function executeInstagramPublish(candidateId: string, accId?: string, token?: string) {
    if (!detail) return;
    setPublishingCandidateId(candidateId);
    try {
      const activeAccId = accId || instagramAccountId.trim() || environment?.instagramAccountId?.trim() || null;
      const activeToken = token || (instagramAccessToken.trim() ? instagramAccessToken.trim() : null);

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
      showError("Instagram publishing failed", { details: String(err) });
    } finally {
      setPublishingCandidateId(null);
    }
  }

  async function handleSaveMetaAndPost() {
    if (!metaModalAccountId.trim()) {
      showWarning("Please enter your Instagram Account ID");
      return;
    }
    if (!metaModalAccessToken.trim()) {
      showWarning("Please enter your Meta Graph API Access Token");
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
      showError("Failed to save credentials", { details: String(err) });
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
    : llmEngine === "groq" ? canUseGroq : false;

  async function run(action: BusyState, task: () => Promise<void>) {
    setBusy(action);
    try {
      await task();
    } catch (err) {
      showError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("idle");
    }
  }

  async function openFolder(path: string) {
    try {
      await invoke("open_folder", { path });
    } catch (err) {
      showError("Could not open folder", { details: String(err) });
    }
  }

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

  async function importMedia() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Media", extensions: ["mp4", "mov", "mp3", "wav", "m4a"] }],
    });
    if (typeof selected !== "string") return;
    setMediaPathToImport(selected);
    setShowStyleModal(true);
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

  async function runAutoPipeline(projectId: string) {
    const env = await invoke<EnvironmentStatus>("environment_status");

    if (transcriptionEngine === "local" && !env.hasLocalWhisperModel) {
      showWarning("Local Whisper is missing. Add model or switch to Cloud in Settings.");
      return;
    }
    if (transcriptionEngine === "deepgram" && !canUseCloudKey) {
      showWarning("Deepgram API Key is missing. Add it in Settings to transcribe.");
      return;
    }

    try {
      setBusy("transcribe");
      await invoke<Transcript>("transcribe_project", {
        projectId,
        provider: transcriptionEngine,
      });
      await refresh(projectId);
      showToast("Transcription complete!");
    } catch (err) {
      showError("Transcription failed", { details: String(err) });
      setBusy("idle");
      return;
    }

    try {
      setBusy("moments");
      await invoke<Candidate[]>("generate_candidates", {
        projectId,
        provider: llmEngine,
        modelName: llmEngine === "local" ? localLlmModel.trim() : (llmEngine === "deepseek" ? (deepseekModel.trim() || null) : null),
        allowDemo: false,
      });
      await refresh(projectId);
      showToast("Viral moments detected!");
    } catch (err) {
      showError("Moment detection failed", { details: String(err) });
    } finally {
      setBusy("idle");
    }
  }

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
      showError("Failed to rename project", { details: String(err) });
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
      showError("Failed to delete project", { details: String(err) });
    }
  }

  async function transcribe() {
    if (!detail) return;
    await run("transcribe", async () => {
      await invoke<Transcript>("transcribe_project", {
        projectId: detail.project.id,
        provider: transcriptionEngine,
      });
      await refresh(detail.project.id);
      showToast("Transcription finished");
    });
  }

  async function moments() {
    if (!detail) return;
    await run("moments", async () => {
      await invoke<Candidate[]>("generate_candidates", {
        projectId: detail.project.id,
        provider: llmEngine,
        modelName: llmEngine === "local" ? localLlmModel.trim() : (llmEngine === "deepseek" ? (deepseekModel.trim() || null) : null),
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
    try {
      await invoke<string>("render_flat_clip_for_candidate", {
        candidateId,
        reframeMode,
        outputDir: clipsSaveDir.trim() || null,
        removeSilence,
        punchZoom,
        studioAudio,
      });
      showToast("Clip rendered successfully!");
    } catch (err) {
      showError("Render failed", { details: String(err) });
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
    try {
      for (const candidate of selected) {
        setRenderingCandidateId(candidate.id);
        await invoke<string>("render_flat_clip_for_candidate", {
          candidateId: candidate.id,
          reframeMode,
          outputDir: clipsSaveDir.trim() || null,
          removeSilence,
          punchZoom,
          studioAudio,
        });
      }
      showToast(`Finished rendering ${selected.length} clips!`);
    } catch (err) {
      showError("Batch render failed", { details: String(err) });
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
        showError("Failed to generate social kit", { details: String(err) });
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
      showError("Failed to regenerate social kit", { details: String(err) });
    } finally {
      setSocialKitLoading(null);
    }
  }

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

  const closeAndSaveSettings = async () => {
    setShowSettings(false);
    try {
      await Promise.all([
        ["deepgram", deepgramKey],
        ["gemini", geminiKey],
        ["openai", openaiKey],
        ["anthropic", anthropicKey],
        ["deepseek", deepseekKey],
        ["groq", groqKey],
        ["openrouter", openrouterKey],
      ].map(([name, value]) => invoke("save_credential", { name, value })));
      await refresh();
      showToast("Settings saved securely");
    } catch (err) {
      showError("Error saving API credentials", { details: String(err) });
    }
  };

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

      <main className="app-shell">
        <ProjectSidebar
          appSection={appSection}
          onSectionChange={handleSectionChange}
          busy={busy}
          environment={environment}
          projects={projects}
          selectedProjectId={detail?.project.id}
          onSelectProject={(id) => {
            if (id) void selectProject(id);
            else setDetail(null);
          }}
          onImportMedia={importMedia}
          onOpenYoutubeModal={() => setYoutubeModalOpen(true)}
          onOpenSettings={() => setShowSettings(true)}
          fileName={fileName}
        />

        <section className="workspace">
          {detail ? (
            <div className="project-workspace">
              <ProjectHeader
                detail={detail}
                transcript={transcript}
                cutCount={cutCount}
                appSection={appSection}
                onSectionChange={handleSectionChange}
                onBack={() => setDetail(null)}
                onRename={renameProject}
                onOpenClipsFolder={() => {
                  const renderedClip = detail?.candidates.map((c) => clipByCandidate.get(c.id)).find((cl) => cl?.outputPath);
                  if (renderedClip?.outputPath) {
                    const parts = renderedClip.outputPath.split(/[\/]/);
                    parts.pop();
                    void openFolder(parts.join("/"));
                    return;
                  }
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
                  void openFolder(clipsSaveDir || defaultFolders?.clipsOutputDir || "");
                }}
                onOpenSettings={() => setShowSettings(true)}
                onRefresh={(id) => void refresh(id)}
                fileName={fileName}
                formatTime={formatTime}
              />

              <div className="studio-grid">
                <TranscriptionPanel
                  transcript={transcript}
                  busy={busy}
                  canTranscribe={canTranscribe}
                  transcriptionEngine={transcriptionEngine}
                  onTranscribe={transcribe}
                  formatTime={formatTime}
                />

                <MomentsPanel
                  detail={detail}
                  appSection={appSection}
                  reframeMode={reframeMode}
                  setReframeMode={setReframeMode}
                  busy={busy}
                  canUseActiveLlm={canUseActiveLlm}
                  onFindMoments={moments}
                  onCutSelected={cutSelected}
                  selectedCount={selectedCount}
                  cutCount={cutCount}
                  momentTab={momentTab}
                  setMomentTab={setMomentTab}
                  selectBatch={selectBatch}
                  punchZoom={punchZoom}
                  setPunchZoom={setPunchZoom}
                  removeSilence={removeSilence}
                  setRemoveSilence={setRemoveSilence}
                  studioAudio={studioAudio}
                  setStudioAudio={setStudioAudio}
                  exportPreset={exportPreset}
                  setExportPreset={setExportPreset}
                  filteredCandidates={filteredCandidates}
                  clipByCandidate={clipByCandidate}
                  instagramPostByCandidate={instagramPostByCandidate}
                  renderingCandidateId={renderingCandidateId}
                  publishingCandidateId={publishingCandidateId}
                  hasFfmpeg={Boolean(environment?.hasFfmpeg)}
                  formatTime={formatTime}
                  toggleCandidate={toggleCandidate}
                  handleOpenSocialKit={handleOpenSocialKit}
                  cutCandidate={cutCandidate}
                  openFolder={openFolder}
                  handlePublishToInstagram={handlePublishToInstagram}
                  onJobComplete={() => {
                    showToast("Render completed!");
                    if (detail) refresh(detail.project.id);
                  }}
                  onJobCancel={() => {
                    showToast("Render cancelled by user");
                    if (detail) refresh(detail.project.id);
                  }}
                />
              </div>
            </div>
          ) : (
            <ProjectsDashboard
              projects={projects}
              appSection={appSection}
              busy={busy}
              environment={environment}
              transcriptionEngine={transcriptionEngine}
              onImportMedia={importMedia}
              onOpenYoutubeModal={() => setYoutubeModalOpen(true)}
              onSelectProject={selectProject}
              onRenameProject={renameProject}
              onDeleteProject={deleteProject}
              fileName={fileName}
              formatTime={formatTime}
              formatDate={formatDate}
            />
          )}
        </section>
      </main>

      <StatusBar environment={environment} canUseCloudKey={canUseCloudKey} />

      {/* Modals */}
      <SettingsModal
        isOpen={showSettings}
        onClose={() => setShowSettings(false)}
        environment={environment}
        transcriptionEngine={transcriptionEngine}
        setTranscriptionEngine={setTranscriptionEngine}
        deepgramKey={deepgramKey}
        setDeepgramKey={setDeepgramKey}
        llmEngine={llmEngine}
        setLlmEngine={setLlmEngine}
        localLlmModel={localLlmModel}
        setLocalLlmModel={setLocalLlmModel}
        anthropicKey={anthropicKey}
        setAnthropicKey={setAnthropicKey}
        deepseekKey={deepseekKey}
        setDeepseekKey={setDeepseekKey}
        deepseekModel={deepseekModel}
        setDeepseekModel={setDeepseekModel}
        geminiKey={geminiKey}
        setGeminiKey={setGeminiKey}
        openaiKey={openaiKey}
        setOpenaiKey={setOpenaiKey}
        groqKey={groqKey}
        setGroqKey={setGroqKey}
        instagramProvider={instagramProvider}
        setInstagramProvider={setInstagramProvider}
        instagramAccountId={instagramAccountId}
        setInstagramAccountId={setInstagramAccountId}
        instagramAccessToken={instagramAccessToken}
        setInstagramAccessToken={setInstagramAccessToken}
        instagramWebhookUrl={instagramWebhookUrl}
        setInstagramWebhookUrl={setInstagramWebhookUrl}
        testInstagramConnection={testInstagramConnection}
        instagramTesting={instagramTesting}
        instagramTestResult={instagramTestResult}
        youtubeSaveDir={youtubeSaveDir}
        setYoutubeSaveDir={setYoutubeSaveDir}
        clipsSaveDir={clipsSaveDir}
        setClipsSaveDir={setClipsSaveDir}
        defaultFolders={defaultFolders}
        reframeMode={reframeMode}
        setReframeMode={setReframeMode}
        punchZoom={punchZoom}
        setPunchZoom={setPunchZoom}
        removeSilence={removeSilence}
        setRemoveSilence={setRemoveSilence}
        studioAudio={studioAudio}
        setStudioAudio={setStudioAudio}
        pullModelDirectly={async (modelName) => {
          showInfo(`Pulling model ${modelName} via Ollama...`);
          try {
            await invoke("pull_ollama_model", { modelName });
            showToast(`Model ${modelName} downloaded!`);
          } catch (e) {
            showError("Failed to pull model", { details: String(e) });
          }
        }}
        browseFolder={browseFolder}
        openFolder={openFolder}
        onClearStorage={async () => {
          if (confirm("Are you sure you want to clear all project storage? All downloaded videos, rendered clips, and project records will be wiped.")) {
            try {
              const res = await invoke<string>("clear_all_storage");
              showToast(res);
              setProjects([]);
              setDetail(null);
            } catch (e) {
              showError("Failed to clear storage", { details: String(e) });
            }
          }
        }}
        onSaveAndClose={closeAndSaveSettings}
      />

      <CaptionStyleModal
        isOpen={showStyleModal}
        selectedStyle={selectedStyle}
        onSelectStyle={setSelectedStyle}
        onConfirm={confirmImport}
        onCancel={() => {
          setShowStyleModal(false);
          setMediaPathToImport(null);
        }}
      />

      <YoutubeImportModal
        isOpen={youtubeModalOpen}
        onClose={() => setYoutubeModalOpen(false)}
        onSuccess={(downloadedPath) => {
          setMediaPathToImport(downloadedPath);
          setShowStyleModal(true);
        }}
        youtubeSaveDir={youtubeSaveDir}
      />

      <SocialKitModal
        candidate={socialKitModalCandidate}
        loading={socialKitLoading === socialKitModalCandidate?.id}
        kitData={socialKitModalCandidate ? socialKitData[socialKitModalCandidate.id] : undefined}
        onClose={() => setSocialKitModalCandidate(null)}
        onRegenerate={handleRegenerateSocialKit}
        onShowToast={showToast}
      />

      <InstagramPublishModal
        isOpen={showMetaModal}
        onClose={() => setShowMetaModal(false)}
        accountId={metaModalAccountId}
        setAccountId={setMetaModalAccountId}
        accessToken={metaModalAccessToken}
        setAccessToken={setMetaModalAccessToken}
        onTestConnection={handleTestMetaConnection}
        testing={metaModalTesting}
        status={metaModalStatus}
        onSaveAndPost={handleSaveMetaAndPost}
        saving={metaModalSaving}
        pendingCandidateId={pendingCandidateIdToPost}
        onOpenExternal={openFolder}
      />
    </div>
  );
}

export function App() {
  return (
    <ErrorProvider>
      <AppContent />
    </ErrorProvider>
  );
}

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
