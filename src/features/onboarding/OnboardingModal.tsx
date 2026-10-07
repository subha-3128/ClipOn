import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Clapperboard,
  Database,
  Cloud,
  Copy,
  Check,
  BadgeCheck,
  Loader2,
} from "lucide-react";
import { EnvironmentStatus } from "../../types";

export interface OnboardingProps {
  environment: EnvironmentStatus | null;
  onComplete: () => void;
  setTranscriptionEngine: (engine: "deepgram" | "local") => void;
  setLlmEngine: (
    engine:
      | "claude"
      | "deepseek"
      | "local"
      | "gemini"
      | "openai"
      | "openrouter"
      | "groq"
  ) => void;
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

export function Onboarding({
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
  const [setupMode, setSetupMode] = useState<
    "choose" | "local" | "cloud" | "downloading"
  >("choose");
  const [selectedModel, setSelectedModel] = useState<string>("llama3.2");
  const [dgKey, setDgKey] = useState(initialDeepgramKey || "");
  const [gmKey, setGmKey] = useState(initialGeminiKey || "");
  const [antKey, setAntKey] = useState(initialAnthropicKey || "");
  const [dsKey, setDsKey] = useState(initialDeepseekKey || "");
  const [grKey, setGrKey] = useState(initialGroqKey || "");
  const [downloadStatus, setDownloadStatus] = useState(
    "Initializing download..."
  );
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [checkingOllama, setCheckingOllama] = useState(false);
  const [copied, setCopied] = useState(false);

  const skipToStudio = () => {
    localStorage.setItem("clipon_onboarded", "true");
    onComplete();
  };

  useEffect(() => {
    if (!dgKey && initialDeepgramKey) setDgKey(initialDeepgramKey);
    if (!gmKey && initialGeminiKey) setGmKey(initialGeminiKey);
    if (!antKey && initialAnthropicKey) setAntKey(initialAnthropicKey);
    if (!dsKey && initialDeepseekKey) setDsKey(initialDeepseekKey);
    if (!grKey && initialGroqKey) setGrKey(initialGroqKey);
  }, [
    environment,
    initialDeepgramKey,
    initialGeminiKey,
    initialAnthropicKey,
    initialDeepseekKey,
    initialGroqKey,
  ]);

  const copyWhisperCommand = () => {
    navigator.clipboard.writeText("pip3 install -U openai-whisper");
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleCloudSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (dgKey.trim()) {
      setTranscriptionEngine("deepgram");
      setDeepgramKey(dgKey.trim());
      localStorage.setItem("clipon_transcription_engine", "deepgram");
    }

    let cleanGm = gmKey.trim();
    if (cleanGm.startsWith("Q.Ab8")) cleanGm = "A" + cleanGm;

    if (cleanGm) {
      setLlmEngine("gemini");
      setGeminiKey(cleanGm);
      localStorage.setItem("clipon_llm_engine", "gemini");
    } else if (antKey.trim()) {
      setLlmEngine("claude");
      setAnthropicKey(antKey.trim());
      localStorage.setItem("clipon_llm_engine", "claude");
    } else if (dsKey.trim()) {
      setLlmEngine("deepseek");
      setDeepseekKey(dsKey.trim());
      localStorage.setItem("clipon_llm_engine", "deepseek");
    } else if (grKey.trim()) {
      setLlmEngine("groq");
      setGroqKey(grKey.trim());
      localStorage.setItem("clipon_llm_engine", "groq");
    }

    try {
      await Promise.all(
        [
          ["deepgram", dgKey],
          ["gemini", cleanGm],
          ["anthropic", antKey],
          ["deepseek", dsKey],
          ["groq", grKey],
        ].map(([name, value]) => invoke("save_credential", { name, value }))
      );
      await refreshEnv();
    } catch (err) {
      console.error("Failed to persist credentials safely:", err);
    }

    localStorage.setItem("clipon_onboarded", "true");
    onComplete();
  };

  const startLocalSetup = async () => {
    setError(null);
    setCheckingOllama(true);
    setDownloadProgress(0);
    await refreshEnv();

    let isOllamaRunning: boolean;
    let currentEnv: EnvironmentStatus | null = null;
    try {
      currentEnv = await invoke<EnvironmentStatus>("environment_status");
      isOllamaRunning = currentEnv.hasOllama;
    } catch {
      isOllamaRunning = false;
    }

    setCheckingOllama(false);

    if (!isOllamaRunning) {
      if (currentEnv && currentEnv.ollamaInstallSupported === false) {
        setError(
          `Ollama was not detected at http://localhost:11434. In-app automatic installation is supported on macOS. On ${currentEnv.platform === "windows" ? "Windows" : "Linux"}, please download and start Ollama from https://ollama.com first.`
        );
        return;
      }
      setSetupMode("downloading");
      setDownloadStatus("Ollama not found. Starting automatic installer...");
      try {
        const unlistenInstall = await listen<string>(
          "ollama-install-status",
          (event) => {
            setDownloadStatus(event.payload);
          }
        );
        await invoke("install_ollama");
        unlistenInstall();
      } catch (err) {
        setError(
          "Automatic installation failed: " +
            String(err) +
            ". Please install it manually from ollama.com."
        );
        setSetupMode("local");
        return;
      }
    }

    setSetupMode("downloading");
    setDownloadStatus("Ollama connected. Initiating model download...");

    try {
      const unlisten = await listen<{
        status: string;
        completed?: number;
        total?: number;
        percentage?: number;
      }>("ollama-pull-progress", (event) => {
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
    <div className="modal-overlay" role="presentation">
      <div
        className="onboarding-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby="onboarding-dialog-title"
      >
        {setupMode === "choose" && (
          <>
            <div className="onboarding-header">
              <div className="brand-mark large">
                <Clapperboard size={36} />
              </div>
              <h2 id="onboarding-dialog-title">Welcome to ClipOn</h2>
              <p>
                Long recording in. Short clips out. Choose how you want to run
                the studio.
              </p>
            </div>

            <div className="onboarding-choices">
              <div
                className="choice-card"
                onClick={() => setSetupMode("local")}
              >
                <div className="choice-icon">
                  <Database size={28} />
                </div>
                <h3>Fully Offline &amp; Private</h3>
                <p>
                  Process everything locally on your machine. 100% private,
                  free, and offline.
                </p>
                <div className="choice-badge local">
                  Offline (Ollama + Whisper)
                </div>
              </div>

              <div
                className="choice-card"
                onClick={() => setSetupMode("cloud")}
              >
                <div className="choice-icon">
                  <Cloud size={28} />
                </div>
                <h3>Cloud APIs</h3>
                <p>
                  Blazing fast cloud transcription &amp; analysis. Minimal local
                  RAM requirements.
                </p>
                <div className="choice-badge cloud">API Keys Required</div>
              </div>
            </div>
            <div
              style={{
                marginTop: "24px",
                display: "flex",
                justifyContent: "center",
              }}
            >
              <button
                type="button"
                className="studio-btn secondary"
                onClick={skipToStudio}
              >
                Skip Setup &amp; Explore Studio
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

            {error && (
              <div
                className="workspace-error-banner"
                style={{ marginBottom: "16px" }}
              >
                {error}
              </div>
            )}

            <div className="setup-steps">
              <div className="setup-step">
                <div className="step-num">1</div>
                <div className="step-body">
                  <h4>Install Python Whisper</h4>
                  <p>
                    Run the following command in terminal to enable local
                    transcription:
                  </p>
                  <div className="code-block-container">
                    <code>pip3 install -U openai-whisper</code>
                    <button
                      type="button"
                      className="copy-btn"
                      onClick={copyWhisperCommand}
                    >
                      {copied ? <Check size={14} /> : <Copy size={14} />}
                      {copied ? "Copied!" : "Copy"}
                    </button>
                  </div>
                  {environment?.hasLocalWhisperModel ? (
                    <span className="step-check success">
                      <BadgeCheck size={14} /> Whisper detected in Python!
                    </span>
                  ) : (
                    <span className="step-check warning">
                      Package 'whisper' not detected yet. Run command above.
                    </span>
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
                      <p>
                        Fast, efficient, excellent hook identification for
                        shorts.
                      </p>
                    </div>

                    <div
                      className={`model-card ${selectedModel === "qwen2.5:3b" ? "active" : ""}`}
                      onClick={() => setSelectedModel("qwen2.5:3b")}
                    >
                      <div className="model-card-header">
                        <h5>Qwen 2.5 3B</h5>
                        <span className="model-size">2.0 GB</span>
                      </div>
                      <p>
                        Optimized for multilingual dialogues and concise hooks.
                      </p>
                    </div>
                  </div>
                  {environment?.ollamaInstallSupported === false &&
                    !environment?.hasOllama && (
                      <p
                        style={{
                          marginTop: "10px",
                          fontSize: "12px",
                          color: "var(--muted)",
                        }}
                      >
                        Note: Automatic installation is macOS only. On{" "}
                        {environment?.platform === "windows"
                          ? "Windows"
                          : "Linux"}
                        , please install and run Ollama from{" "}
                        <a
                          href="https://ollama.com"
                          target="_blank"
                          rel="noreferrer"
                          style={{
                            color: "var(--accent)",
                            textDecoration: "underline",
                          }}
                        >
                          ollama.com
                        </a>
                        .
                      </p>
                    )}
                </div>
              </div>
            </div>

            <div className="onboarding-actions">
              <button
                type="button"
                className="studio-btn secondary"
                onClick={() => setSetupMode("choose")}
              >
                Back
              </button>
              <button
                type="button"
                className="studio-btn secondary"
                onClick={skipToStudio}
              >
                Skip Setup
              </button>
              <button
                type="button"
                className="studio-btn primary"
                onClick={startLocalSetup}
                disabled={checkingOllama}
              >
                {checkingOllama ? <Loader2 className="spin" size={16} /> : null}
                {checkingOllama
                  ? "Connecting Ollama..."
                  : "Download & Finish Setup"}
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

            {error && (
              <div
                className="workspace-error-banner"
                style={{ marginBottom: "16px" }}
              >
                {error}
              </div>
            )}

            <div className="form-stack">
              <div className="settings-field-group">
                <label>Deepgram API Key (Transcription)</label>
                <input
                  type="password"
                  value={dgKey}
                  onChange={(e) => setDgKey(e.target.value)}
                  placeholder={
                    environment?.hasDeepgramKey
                      ? "Saved securely"
                      : "Deepgram API Key"
                  }
                />
              </div>

              <div className="settings-field-group">
                <label>Google Gemini API Key (Recommended)</label>
                <input
                  type="password"
                  value={gmKey}
                  onChange={(e) => setGmKey(e.target.value)}
                  placeholder={
                    environment?.hasGeminiKey
                      ? "Saved securely"
                      : "Google Gemini API Key"
                  }
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
              <button
                type="button"
                className="studio-btn secondary"
                onClick={() => setSetupMode("choose")}
              >
                Back
              </button>
              <button
                type="button"
                className="studio-btn secondary"
                onClick={skipToStudio}
              >
                Skip for Now
              </button>
              <button type="submit" className="studio-btn primary">
                Save &amp; Launch Studio
              </button>
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
              <div
                className="progress-fill"
                style={{ width: `${downloadProgress}%` }}
              />
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
