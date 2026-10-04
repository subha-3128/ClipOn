# ClipOn ✂️

[![GitHub](https://img.shields.io/badge/Creator-subha--3128-181717?style=flat&logo=github)](https://github.com/subha-3128)
[![Repository](https://img.shields.io/badge/GitHub-subha--3128%2FClipOn-10b981?style=flat&logo=github)](https://github.com/subha-3128/ClipOn)

> **Created & Maintained by [@subha-3128](https://github.com/subha-3128)**  
> 🔗 **Official Repository**: [https://github.com/subha-3128/ClipOn](https://github.com/subha-3128/ClipOn)

---

**ClipOn** is an ultra-fast, local-first desktop application designed for content creators, podcasters, and video editors to transform long-form recordings and YouTube videos into high-converting, viral vertical short-form clips (**9:16 YouTube Shorts, Instagram Reels, and TikToks**) powered by AI moment ranking.

Built with **Tauri 2 + React 19 + TypeScript + Rust + SQLite + Apple Silicon VideoToolbox**.

---

## ⚡ Core Features

- **🚀 Apple Silicon GPU Acceleration**: Native hardware-accelerated video transcoding via Apple VideoToolbox (`h264_videotoolbox`). Renders full-resolution 1080x1920 60fps vertical clips in 1–3 seconds with near-zero CPU load.
- **📱 Smart 9:16 Vertical Reframe**:
  - **9:16 Center Crop (Default)**: Converts 16:9 widescreen videos into standard vertical 9:16 full-screen shorts with precise subject framing.
  - **9:16 Center Crop**: Direct center cut for solo speaker podcasts.
  - **16:9 Original**: Retains original aspect ratio.
- **✨ AI Social Publishing Kit**:
  - Auto-generates **3 High-CTR Viral Titles** (Curiosity, Value, Controversy) per clip.
  - Trending topic hashtags with 1-click copy.
  - Complete post caption & description + Call to Action.
  - 1-click **"Copy Complete Social Package"** ready for immediate publishing to TikTok, Reels, and YouTube Shorts.
- **🤖 Multi-LLM Moment Ranking**:
  - Compatible with **Ollama** (100% offline & private local models like LLaMA 3.2, Qwen 2.5), **DeepSeek**, **Google Gemini**, **Anthropic Claude**, **OpenAI**, and **Groq**.
  - Ranks clips by hook strength, virality score, and audience retention potential.
- **🎙️ Dual-Engine Transcription**:
  - **Offline Local Whisper**: Private, free transcription via local Whisper model.
  - **Deepgram Nova-2**: High-speed cloud transcription with word-level timestamps.
- **📥 Direct YouTube Importer**:
  - Paste any YouTube link to download and analyze locally.
  - Automatic Creative Commons / copyright license verification.
- **📁 Native Folder Management**:
  - Native macOS directory picker (`Browse...`) to choose custom download and clip output directories.
  - Instant 1-click **"Show in Finder"** for any clip or project folder.
- **🎛️ Minimalist Pro Studio UI/UX**:
  - Monochrome, minimalist **All Projects** dashboard with real-time search.
  - 4-Stage visual pipeline tracker: `1. Source Loaded` → `2. Interactive Transcript` → `3. Viral Moments` → `4. Rendered Clips`.
  - Split-screen workspace with interactive dialogue search, batch clip selection, and live cutting status.

---

## 📂 Project File Structure

```text
ClipOn/
├── .env.example                     # Environment template for API keys & folder paths
├── .gitignore                       # Git ignore rules (node_modules, target, dist, .env)
├── README.md                        # Documentation & setup guide
├── index.html                       # HTML entry point with Plus Jakarta Sans & JetBrains Mono
├── package.json                     # Node.js project manifest & scripts
├── package-lock.json                # Locked dependency tree
├── tsconfig.json                    # TypeScript compiler configuration (ES2022, React JSX)
├── vite.config.ts                   # Vite bundler configuration (dev server on 127.0.0.1:1420)
│
├── src/                             # Frontend application source (React 19 + TypeScript)
│   ├── main.tsx                     # Main UI orchestrator, state management, & modal systems
│   │                                # (Dashboard, Studio Workspace, Settings, Social Kit, YouTube)
│   └── styles.css                   # Cohesive dark-mode design system & micro-interactions
│
├── src-tauri/                       # Desktop native layer (Rust + Tauri v2)
│   ├── Cargo.toml                   # Rust package manifest & dependencies
│   ├── Cargo.lock                   # Locked Rust dependencies
│   ├── build.rs                     # Tauri build script
│   ├── tauri.conf.json              # App configuration (window dimensions, bundle identifier, icons)
│   │
│   ├── capabilities/                # Tauri v2 security & permission manifests
│   │   └── default.json             # Core window, dialog, and event capabilities
│   │
│   ├── icons/                       # Native desktop icons
│   │   ├── icon.icns                # macOS application icon bundle (Apple VideoToolbox)
│   │   ├── icon.ico                 # Windows application icon
│   │   ├── icon.png                 # Master 1024x1024 transparent icon
│   │   └── *.png                    # 32x32, 128x128 desktop app icons
│   │
│   └── src/                         # Rust backend source code
│       ├── main.rs                  # Native application entry point
│       ├── lib.rs                   # Tauri commands handler, app lifecycle, & IPC router
│       ├── media.rs                 # FFmpeg/FFprobe runner, Apple Silicon VideoToolbox GPU pipeline
│       ├── llm.rs                   # Prompt engineering & LLM integration (Ollama, Claude, Gemini, etc.)
│       ├── transcription.rs         # Speech-to-text runner (Whisper local & Deepgram cloud)
│       ├── db.rs                    # Embedded SQLite database schema, migrations, & queries
│       └── models.rs                # Rust data structures (Project, Candidate, Clip, SocialKit)
```

---

## 🛠️ System Prerequisites

ClipOn requires **FFmpeg & FFprobe** on your system `PATH`:

### macOS (Homebrew)
```bash
brew install ffmpeg yt-dlp
```

### Optional Offline Engines:
- **Local Whisper**: `pip3 install -U openai-whisper`
- **Local LLM**: Install [Ollama](https://ollama.com) (`ollama run llama3.2`)

---

## 🚀 Getting Started

### 1. Configure Environment
Copy `.env.example` to `.env` and add your preferred API keys:
```bash
cp .env.example .env
```

```env
# Cloud Transcription (Optional if using Local Whisper)
DEEPGRAM_API_KEY=your_deepgram_api_key

# Viral Moments LLM (Optional if using Local Ollama)
GEMINI_API_KEY=your_gemini_api_key
DEEPSEEK_API_KEY=your_deepseek_api_key
ANTHROPIC_API_KEY=your_anthropic_api_key
GROQ_API_KEY=your_groq_api_key
OPENAI_API_KEY=your_openai_api_key
LLM_PROVIDER=gemini
```

### 2. Install Dependencies
```bash
npm install
```

### 3. Run in Development Mode
```bash
npm run tauri:dev
```
> This starts the Vite dev server and launches the native macOS desktop app window with hot-reloading enabled.

### 4. How to Close / Stop the App
- **From GUI**: Press **`⌘ + Q`** (`Command + Q`) or click the red close button on the top-left of the window.
- **From Terminal**: Press **`Ctrl + C`** in your terminal window.

---

## 📦 Building Standalone App (`.dmg` / `.app`)

To generate an optimized, standalone macOS installer:
```bash
npm run tauri:build
```
Your ready-to-install `.dmg` package will be created in:
```bash
src-tauri/target/release/bundle/dmg/
```

---

---

## 👤 Author & Credits

- **Creator & Lead Developer**: [@subha-3128](https://github.com/subha-3128)
- **GitHub Repository**: [https://github.com/subha-3128/ClipOn](https://github.com/subha-3128/ClipOn)

## 📄 License
MIT License. Created for high-velocity video creators and editors.
