# ClipOn ✂️

[![GitHub](https://img.shields.io/badge/Creator-subha--3128-181717?style=flat&logo=github)](https://github.com/subha-3128)
[![Repository](https://img.shields.io/badge/GitHub-subha--3128%2FClipOn-10b981?style=flat&logo=github)](https://github.com/subha-3128/ClipOn)
[![Platform](<https://img.shields.io/badge/Platform-macOS%20(Apple%20Silicon)-blue?style=flat&logo=apple>)](https://github.com/subha-3128/ClipOn)

> **Created & Maintained by [@subha-3128](https://github.com/subha-3128)**  
> 🔗 **Official Repository**: [https://github.com/subha-3128/ClipOn](https://github.com/subha-3128/ClipOn)

---

**ClipOn** is an ultra-fast, local-first desktop application designed for content creators, video editors, and digital storytellers to transform long-form recordings and YouTube videos into high-converting, viral vertical short-form clips (**9:16 YouTube Shorts, Instagram Reels, and TikToks**) powered by AI moment ranking and adaptive multi-person dynamic reel reframing.

Built with **Tauri 2 + React 19 + TypeScript + Rust + SQLite + Apple Silicon VideoToolbox + Apple Vision**.

---

## ⚡ Core Features

- **🚀 Apple Silicon GPU Acceleration**: Native hardware-accelerated video transcoding via Apple VideoToolbox (`h264_videotoolbox`). Renders full-resolution 1080x1920 60fps vertical clips in seconds with near-zero CPU load.
- **🎙️ Smart Face Tracking & AI Reframing (16:9 → 9:16)**:
  - **Smart Face Track**: Automatic horizontal pan tracking centered on the speaker using Apple Vision face detection with temporal smoothing.
  - **Vertical Center Crop**: Clean standard 9:16 crop.
  - **Original Aspect Ratio**: Preserves full source frame composition.
  - **Kinetic Burned-in Subtitles**: Dynamic ASS word-level highlighted subtitles placed in safe lower-third margins.
- **✨ AI Social Publishing Kit**:
  - Auto-generates **High-CTR Viral Titles** (Curiosity, Value, Controversy) per clip.
  - Trending topic hashtags with 1-click copy.
  - Complete post caption & description + Call to Action.
  - 1-click **"Copy Complete Social Package"** ready for TikTok, Reels, and YouTube Shorts.
- **🔒 Secure OS Keyring Credential Storage**:
  - API keys and tokens are never passed through frontend IPC arguments or stored in plaintext database tables. All sensitive keys are encrypted and stored via native OS Keyring.
- **🤖 Multi-LLM Moment Ranking**:
  - Compatible with **Ollama** (100% offline & private local models like LLaMA 3.2, Qwen 2.5), **DeepSeek**, **Google Gemini**, **Anthropic Claude**, **OpenAI**, **OpenRouter**, and **Groq**.
  - Ranks clips by hook strength, virality score, and audience retention potential.
- **🎙️ Dual-Engine Transcription**:
  - **Offline Local Whisper**: Private, free transcription via local Whisper model.
  - **Deepgram Nova-2**: High-speed cloud transcription with word-level timestamps.
- **📥 Direct YouTube Importer & Compliance**:
  - Paste any YouTube link to download and analyze locally.
  - Automatic Creative Commons / copyright license verification and user Terms of Service confirmation.
- **🛑 Bounded Render Queue & Real Cancellation**:
  - Bounded concurrency queue (max 2 parallel renders) preventing system resource starvation.
  - True instant child process cancellation via OS signals (SIGTERM/SIGKILL) with automatic partial artifact cleanup.
- **⌨️ Keyboard Shortcuts**:
  - `⌘ + I` / `Ctrl + I`: Import Media
  - `⌘ + Y` / `Ctrl + Y`: Import from YouTube
  - `⌘ + ,` / `Ctrl + ,`: Open Studio Configuration
  - `⌘ + R` / `Ctrl + R`: Refresh Studio State
  - `Escape`: Close any active modal dialog

---

## 🛠️ System Prerequisites & Platform Support

### Supported Platforms

- **macOS (Apple Silicon M1/M2/M3/M4 recommended)**: Fully supported with hardware-accelerated VideoToolbox rendering and Apple Vision face tracking.
- **macOS (Intel x86_64)**: Supported with CPU fallback or Intel QuickSync.
- _Windows / Linux_: Hardware-accelerated encoding via NVENC/VAAPI and standard vertical cropping; local Whisper/Ollama are platform-agnostic.

### Required Binaries

ClipOn requires **FFmpeg & FFprobe** on your system `PATH`:

```bash
# macOS (Homebrew)
brew install ffmpeg yt-dlp
```

### Optional Offline Engines:

- **Local Whisper**: `pip3 install -U openai-whisper`
- **Local LLM**: Install [Ollama](https://ollama.com) (`ollama run llama3.2`)

---

## 📂 Project Architecture

```text
ClipOn/
├── src/                             # Frontend application source (React 19 + TypeScript)
│   ├── features/                    # Modular feature modules
│   │   ├── error/                   # Centralized error provider & toast recovery
│   │   ├── export/                  # Export preset configuration & selector
│   │   ├── jobs/                    # Real-time job progress bar & cancel button
│   │   ├── moments/                 # Candidate cards & viral moments panel
│   │   ├── onboarding/              # First-run guided setup modal
│   │   ├── projects/                # Sidebar, header & all-projects dashboard
│   │   ├── rendering/               # Caption style modal & render controls
│   │   ├── settings/                # Unified settings & diagnostic panel
│   │   ├── social/                  # AI Social Kit & Instagram Reels publisher
│   │   ├── system/                  # System status bar & diagnostics
│   │   └── youtube/                 # YouTube import & terms compliance modal
│   ├── types/                       # Shared TypeScript interfaces & error contracts
│   ├── main.tsx                     # Top-level coordinator & lifecycle manager
│   └── styles.css                   # Dark-mode professional design system
│
└── src-tauri/                       # Desktop native layer (Rust + Tauri v2)
    ├── bin/                         # Native helper binaries
    │   └── clipon-face-tracker      # Apple Vision Swift face tracking binary
    └── src/                         # Rust backend
        ├── analysis_cache.rs        # Parameter-hashed cache for face tracking & LLM
        ├── credentials.rs           # OS Keyring secure credential storage
        ├── db.rs                    # SQLite schema with versioned migrations & foreign keys
        ├── http_client.rs           # Resilient HTTP client with retry & rate-limit backoff
        ├── jobs.rs                  # Bounded render queue with process cancellation
        ├── llm.rs                   # AI moment ranking (Ollama, Claude, DeepSeek, etc.)
        ├── media/                   # FFmpeg rendering, face tracker, VideoToolbox encoder, presets
        └── transcription.rs         # Local Whisper & Deepgram integration
```

---

## 🚀 Getting Started

### 1. Install Dependencies

```bash
npm install
```

### 2. Configure Environment (Optional)

Copy `.env.example` to `.env` or configure keys directly via in-app Settings:

```bash
cp .env.example .env
```

### 3. Run in Development Mode

```bash
npm run tauri:dev
```

### 4. Build Standalone Installer (`.dmg` / `.app`)

```bash
npm run tauri:build
```

Your package will be created in `src-tauri/target/release/bundle/dmg/`.

---

## 🧪 Testing

```bash
# Run all Rust unit tests
cargo test --manifest-path src-tauri/Cargo.toml --lib

# Run Golden reframe integration tests (Original, VerticalCrop, SmartFaceTrack)
cargo test --manifest-path src-tauri/Cargo.toml --test reframe_golden

# Run Frontend unit & accessibility tests
npm test

# Verify Frontend TypeScript & production bundle
npm run build

# Run comprehensive end-to-end smoke test
./scripts/smoke_test.sh
```

---

## 👤 Author & Credits

- **Creator & Lead Developer**: [@subha-3128](https://github.com/subha-3128)
- **GitHub Repository**: [https://github.com/subha-3128/ClipOn](https://github.com/subha-3128/ClipOn)

## 📄 License

MIT License. Created for high-velocity video creators and editors.
