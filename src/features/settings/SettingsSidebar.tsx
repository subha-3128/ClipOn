import React from "react";
import {
  Sparkles,
  AudioLines,
  Film,
  Share2,
  FolderOpen,
  Cpu,
} from "lucide-react";
import { SettingsTab, EnvironmentStatus } from "../../types";

interface SettingsSidebarProps {
  activeTab: SettingsTab;
  onSelectTab: (tab: SettingsTab) => void;
  environment: EnvironmentStatus | null;
}

interface NavItemConfig {
  id: SettingsTab;
  label: string;
  sub: string;
  icon: React.ComponentType<{ size?: number; className?: string }>;
  isConfigured?: boolean;
}

export function SettingsSidebar({
  activeTab,
  onSelectTab,
  environment,
}: SettingsSidebarProps) {
  const hasAnyLlm = Boolean(
    environment?.hasGeminiKey ||
    environment?.hasOpenaiKey ||
    environment?.hasAnthropicKey ||
    environment?.hasDeepseekKey ||
    environment?.hasGroqKey ||
    environment?.hasOllama
  );

  const hasAnyTranscription = Boolean(
    environment?.hasDeepgramKey || environment?.hasLocalWhisperModel
  );

  const hasAnySocial = Boolean(
    environment?.hasInstagramToken || environment?.hasYoutubeConfig
  );

  const navItems: NavItemConfig[] = [
    {
      id: "ai",
      label: "AI & Models",
      sub: "Hook detection & LLM providers",
      icon: Sparkles,
      isConfigured: hasAnyLlm,
    },
    {
      id: "transcription",
      label: "Transcription",
      sub: "Deepgram & Whisper offline",
      icon: AudioLines,
      isConfigured: hasAnyTranscription,
    },
    {
      id: "video",
      label: "Video & Reframe",
      sub: "9:16 crop, ASD & dynamic audio",
      icon: Film,
      isConfigured: true,
    },
    {
      id: "social",
      label: "Social Accounts",
      sub: "Instagram Reels & YouTube Shorts",
      icon: Share2,
      isConfigured: hasAnySocial,
    },
    {
      id: "storage",
      label: "Storage & Paths",
      sub: "Clips folders & download dirs",
      icon: FolderOpen,
      isConfigured: true,
    },
    {
      id: "system",
      label: "System Diagnostics",
      sub: "GPU encoders & tools health",
      icon: Cpu,
      isConfigured: Boolean(environment?.hasFfmpeg),
    },
  ];

  return (
    <nav className="settings-sidebar-nav" aria-label="Settings categories">
      {navItems.map((item) => {
        const Icon = item.icon;
        const isActive = activeTab === item.id;
        return (
          <button
            key={item.id}
            type="button"
            className={`settings-nav-item ${isActive ? "active" : ""}`}
            onClick={() => onSelectTab(item.id)}
            aria-selected={isActive}
            role="tab"
          >
            <div className="nav-item-icon-wrapper">
              <Icon size={16} />
            </div>

            <div className="nav-item-text">
              <span className="nav-item-title">{item.label}</span>
              <span className="nav-item-sub">{item.sub}</span>
            </div>

            {item.isConfigured && (
              <span
                className="nav-item-dot configured"
                title="Configured and active"
              />
            )}
          </button>
        );
      })}
    </nav>
  );
}
