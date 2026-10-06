# ClipOn Testing Strategy & Test Suite Guide

## 1. Test Architecture Overview

ClipOn incorporates a multi-tiered test suite ensuring correctness across database migrations, async job queues, media reframing, and frontend user interfaces.

```text
├── Rust Unit Tests           (src-tauri/src/**)       -> 38+ unit tests
├── Database Migrations Tests (src-tauri/src/db.rs)    -> Foreign keys, upgrade idempotence
├── Integration Tests         (src-tauri/tests/**)     -> Real media renders & golden verification
│   ├── podcast_layouts.rs                             -> 1P, 2P, 3P layout continuity & ASS subtitles
│   └── reframe_golden.rs                              -> Golden tests across all 4 reframe modes
└── Frontend Typecheck/Build  (src/**)                 -> TypeScript 5.8 & Vite production bundle
```

---

## 2. Running the Test Suites

### Unit Tests

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

Runs unit tests for:

- Database versioned migrations (001, 002) and foreign key cascades.
- Analysis cache key computation and cache invalidation.
- Bounded job queue semaphore concurrency and cancellation lifecycle.
- Hardware capability caching with `OnceLock`.
- FFmpeg path escaping (colons, spaces, apostrophes, brackets).
- HTTP client retry backoff on 429 rate-limits and 503 service unavailable.
- RenderPlan, TimelineSegment, and ReframePlan validation.

### Golden Reframe Integration Tests

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test reframe_golden
```

Verifies rendering across all 4 modes on real video footage:

1. `Original`: Preserves source aspect ratio and audio stream.
2. `VerticalCrop`: Produces 1080x1920 9:16 video with audio.
3. `SmartFaceTrack`: Produces 1080x1920 9:16 video centered on speaker face.
4. `PodcastSplit`: Produces 1080x1920 9:16 split screen with audio.

### Dynamic Podcast Layout Continuity Tests

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test podcast_layouts
```

Validates continuous face tracking, layout confirmation hysteresis, minimum segment durations, and seam-line centered ASS captions.

### Frontend Typechecking & Production Build

```bash
npm run build
```

Executes `tsc` and `vite build` to guarantee zero TypeScript or bundle errors.

---

## 3. End-to-End Smoke Test Script (Issue #36)

To execute an automated smoke test across the entire pipeline:

```bash
bash scripts/smoke_test.sh
```

This script:

1. Validates prerequisites (`ffmpeg`, `ffprobe`, Node.js, Rust/Cargo).
2. Verifies frontend build.
3. Runs all Rust unit tests.
4. Executes the golden reframe integration tests against `testvideo2.mp4`.
5. Confirms output artifacts are valid MP4 files with video and audio streams.
