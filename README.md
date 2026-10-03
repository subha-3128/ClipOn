# ClipOn ✂️

**ClipOn** is a high-performance, local-first desktop application for turning long-form videos into viral, high-converting vertical short-form clips (9:16 Shorts, Reels, and TikToks) with AI-powered viral moment ranking.

Built with **Tauri 2 + React + TypeScript + Rust + SQLite**.

---

## ⚡ Features

- **🚀 Apple Silicon GPU Acceleration**: Native hardware-accelerated video rendering via Apple VideoToolbox (`h264_videotoolbox`), exporting clips in 1–3 seconds with near-zero CPU load.
- **📱 Smart 9:16 Vertical Reframe**:
  - **9:16 Smart Blur (Default)**: Converts 16:9 widescreen videos into vertical shorts with a high-definition blurred mirror background and sharp centered video.
  - **9:16 Center Crop**: Direct center cut for solo speaker podcasts.
  - **16:9 Original**: Retains original aspect ratio.
- **✨ AI Social Publishing Kit**:
  - Auto-generates **3 High-CTR Viral Titles** (Curiosity, Value, Controversy) per clip.
  - Trending topic hashtags with 1-click copy.
  - Post caption & description + Call to Action.
  - 1-click **"Copy Full Post Package"** for rapid publishing to YouTube Shorts, TikTok, and Instagram Reels.
- **🤖 Multi-LLM Moment Detection**:
  - Supports **DeepSeek**, **Gemini**, **Claude**, **OpenAI**, **Groq**, and local **Ollama** models for ranking moments by hook strength and virality score.
- **🎙️ Fast Cloud & Offline Transcription**:
  - Deepgram cloud transcription with word-level timestamps.
  - Offline local Whisper support.
- **📥 Direct YouTube Importer**:
  - Paste any YouTube link to analyze and download locally with copyright checking.
- **📁 Custom File Locations**:
  - Choose custom download and clip export folders with 1-click Finder access.

---

## 🛠️ Prerequisites

To run ClipOn, ensure **FFmpeg & FFprobe** are installed on your system `PATH`:

### macOS
```bash
brew install ffmpeg
```

---

## 🚀 Getting Started

### 1. Configure Environment
Copy `.env.example` to `.env` and add your API keys:
```bash
cp .env.example .env
```

```env
DEEPGRAM_API_KEY=your_deepgram_key
GEMINI_API_KEY=your_gemini_key
DEEPSEEK_API_KEY=your_deepseek_key
LLM_PROVIDER=gemini
```

### 2. Run in Development
```bash
npm install
npm run tauri:dev
```

### 3. Build Production App
```bash
npm run tauri:build
```
The output `.app` and `.dmg` installers will be generated under `src-tauri/target/release/bundle/`.

---

## 📄 License
MIT License.
