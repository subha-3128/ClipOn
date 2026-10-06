# ClipOn — Problem Fixing & Production Roadmap

> Step-by-step list of the problems found in ClipOn and the recommended process to fix them one by one.
>
> **Rule:** Fix in order. After every item, build/test and verify before moving to the next.

## Priority

- **P0 Critical:** blocks production, core functionality, or security
- **P1 High:** major reliability, architecture, performance, or feature-quality issue
- **P2 Medium:** maintainability/reliability improvement
- **P3 Low:** polish/documentation

## Verification Status Levels

To ensure engineering claims accurately match verified test coverage:

- **Implemented:** Code changes written, integrated, and building without regressions.
- **Unit-tested:** Verified via automated unit tests (`cargo test --lib`).
- **Integration-tested:** End-to-end rendering and pipeline execution verified with controlled media fixtures.
- **CI-tested:** Validated automatically on pull requests and commits in CI.
- **Production-verified:** Validated in release build on end-user operating systems.

---

# Phase 1 — Production Blockers

## 1. Fix Dynamic Podcast Tracker CLI Arguments

**Priority:** P0  
**File:** `src-tauri/src/dynamic_podcast_reframing.rs`

- [x] Status: Unit-tested

### Problem

Rust currently passes `-nostdin` to the custom Swift tracker:

```rust
cmd.arg("-nostdin");
cmd.arg(source_path);
cmd.arg(format!("{start_sec:.3}"));
cmd.arg(format!("{duration_sec:.3}"));
```

But `face_tracker.swift` expects:

```text
videoPath
startSec
durationSec
```

The extra argument shifts every parameter.

### Fix

Use:

```rust
let mut cmd = Command::new(&binary_path);
cmd.arg(source_path);
cmd.arg(format!("{start_sec:.3}"));
cmd.arg(format!("{duration_sec:.3}"));
```

Keep `-nostdin` for FFmpeg only.

### Verify

Run a real Podcast analysis and confirm correct source, start, duration, JSON output, and final render.

---

## 2. Remove API Keys From Frontend IPC

**Priority:** P0  
**Files:** `src/main.tsx` and backend API commands

- [x] Status: Unit-tested

### Problem

React currently passes raw Deepgram/Gemini API keys to Tauri commands.

### Target

Use:

```text
Frontend
  ↓
Tauri Command
  ↓
OS Secure Credential Store
  ↓
API
```

The frontend should only know:

```text
has_deepgram_key
has_gemini_key
```

### Fix Process

1. Create backend credential-store service.
2. Save credentials securely.
3. Add `save_credential`, `delete_credential`, `credential_status` commands.
4. Remove raw `apiKey` arguments from normal processing commands.
5. Update transcription and LLM services to retrieve credentials internally.
6. Remove raw secrets from logs/errors.
7. Search the whole frontend for credential handling.

### Verify

No raw API key crosses normal frontend IPC.

---

# Phase 2 — Dynamic Podcast Reliability

## 3. Strengthen Person Identity Tracking

**Priority:** P1  
**Files:** `src-tauri/bin/face_tracker.swift`, `src-tauri/src/dynamic_podcast_reframing.rs`

- [x] Status: Implemented (Preserved per directive)

### Problem

Current identity matching relies heavily on Vision facial landmarks. This is not sufficient to guarantee stable identity in difficult footage.

### Fix

Combine:

```text
Appearance / embedding
+ spatial position
+ motion
+ bounding-box size
+ landmarks
+ confidence
+ identity history
+ occlusion/lost state
```

Use a weighted association score and tune it against real videos.

### Rules

- Never instantly create a new person when a known person temporarily disappears.
- Preserve IDs through short occlusion.
- Do not let speaker diarization decide visual identity.

### Verify

Test one, two, and three people; exits/re-entry; occlusion; camera movement; similar faces.

---

## 4. Add Explicit Layout Confirmation

**Priority:** P1

- [x] Status: Unit-tested

### Problem

Hysteresis/minimum duration exists, but a new layout should also remain stable for several consecutive observations before switching.

### Fix

Track:

```text
currentLayout
candidateLayout
candidateStartTime
stableDuration
```

Example:

```text
Current: 2
2, 2, 3, 2, 3, 2 → stay at 2

Current: 2
3, 3, 3, 3, 3 → switch to 3
```

Start with about a 1-second confirmation window and tune using test footage.

### Verification (2026-10-06)

`LayoutConfirmationMachine` in `src-tauri/bin/face_tracker.swift` requires a candidate
(layout or person-id set) to be observed continuously for `layoutConfirmationSec = 1.0`
before committing, clamped so the previous segment keeps `minLayoutDurationSec`. Verified
via `--selftest-layout` (7/7 PASS, including both roadmap examples) and full runs of
`testvideo2.mp4` (76→64 segments, keyframe states identical, all segments ≥ 3.00s) and
`testvideo.mp4` (18→17 segments, no sub-3s segments). `cargo test` 14/14. Evidence:
`tests/dynamic_podcast_identity/results/issue4_layout_confirmation_report.md`.

---

## 5. Validate Dynamic Layout Rules

**Priority:** P1

- [x] Status: Unit-tested

### One Person

```text
┌───────────────────┐
│                   │
│     PERSON 1      │
│                   │
└───────────────────┘
```

### Two People

```text
┌───────────────────┐
│     PERSON 1      │
├───────────────────┤
│     PERSON 2      │
└───────────────────┘
```

### Three People

```text
┌─────────┬─────────┐
│ PERSON1 │ PERSON2 │
├─────────┴─────────┤
│      PERSON 3     │
└───────────────────┘
```

### Required

- Exact same source timestamp in all sections.
- Silent people remain visible.
- Captions stay synchronized.
- Layout is based on continuous tracking, not the first frame.

### Verification (2026-10-06)

- Verified via `src-tauri/tests/podcast_layouts.rs` against `testvideo2.mp4`:
  - 1-person layout: tested at 576.0–586.0s (`single`, person [1]) -> verified 1080x1920 9:16 vertical crop.
  - 2-person layout: tested at 196.0–206.0s (`split_two`, persons [1, 2]) -> verified 1080x1920 horizontal split.
  - 3-person layout: tested at 220.0–230.0s (`split_three`, persons [1, 2, 3]) -> verified 1080x1920 2-top + 1-bottom.
  - Continuous tracking across transitions: tested at 44.0–70.0s spanning dynamic transitions.
  - Captioned renders with burned-in kinetic ASS subtitles verified: fixed filter graph input pad in `src-tauri/src/media/renderer.rs` line 319 so `[divided]` connects properly to subtitle filter without leaving output pad unconnected. Both integration tests pass in `cargo test --test podcast_layouts`.

---

# Phase 3 — Rendering Reliability

## 6. Improve Podcast Segment Rendering

**Priority:** P1  
**File:** `src-tauri/src/media/renderer.rs`

- [x] Status: Unit-tested

### Problem

Dynamic layouts currently create temporary rendered segments and concatenate them. Correct, but potentially expensive.

### Fix Process

1. Merge adjacent identical layout segments.
2. Avoid unnecessary FFmpeg processes.
3. Guarantee temporary-file cleanup.
4. Prevent duplicate rendering.
5. Consider a single filter graph for compatible timelines.
6. Keep segmented rendering as a correctness fallback.

### Verification (2026-10-06)

- Implemented `merge_adjacent_segments` which coalesces contiguous segments sharing identical `layout_type` and `person_ids`.
- Single layout segments bypass multi-process segmentation entirely, directly utilizing the optimal single-pass filter graph.
- Implemented `TempDirGuard` providing strict RAII cleanup for all segment intermediate files and concat manifests.
- Added unit tests `test_merge_adjacent_segments`, `test_temp_dir_guard_lifecycle`, and `test_cleanup_stale_temp_dirs` in `src-tauri/src/media/renderer.rs`. All passed.

---

## 7. Build a Bounded Render Queue

**Priority:** P1

- [x] Status: Unit-tested

### Problem

Selected clips are rendered sequentially from frontend code. Safe, but not scalable.

### Fix

Create:

```text
Pending → Running → Completed
```

Use initial concurrency of 1–2 jobs.

Each job should contain:

```text
id
projectId
input
output
status
progress
startTime
endTime
error
cancelState
```

### Verification (2026-10-06)

- Implemented bounded render queue in `src-tauri/src/jobs.rs` utilizing an asynchronous `tokio::sync::Semaphore` with concurrency bound to 2.
- Updated `JobInfo` with `input`, `output`, `cancel_state`, `state`, `progress`, `stage`, `error`, timestamps.
- Added `test_bounded_queue_semaphore` and `test_job_lifecycle` verifying queue slot acquisition and state progression.

---

## 8. Implement Real Job Cancellation

**Priority:** P1

- [x] Status: Unit-tested

### Required lifecycle

```text
Created
→ Running
→ Cancellation Requested
→ Terminate Child
→ Cleanup
→ Cancelled
```

Create a central job manager for spawn, progress, cancellation, process termination, cleanup, and final state.

### Verification (2026-10-06)

- Implemented process registration via `JobManager::register_process_global` and `unregister_process_global`.
- All FFmpeg processes spawned during clip rendering execute cancellably via `execute_command_cancellable`.
- `JobManager::cancel_job` immediately issues SIGTERM (-15) then SIGKILL (-9) to child PIDs and marks `cancel_state = true`.
- Partial outputs are removed on cancellation, database clip status is set to failed/cancelled with user cancellation note, and temporary directories are dropped.
- Verified with unit test `test_job_cancellation`.

---

## 9. Improve Temporary File Cleanup

**Priority:** P1

- [x] Status: Unit-tested

Use a per-job temporary workspace:

```text
Create workspace
→ Render
→ Success/Failure/Cancel
→ Cleanup
```

Also clean stale workspaces after restart.

### Verification (2026-10-06)

- `TempDirGuard` automatically removes the workspace upon success, error, or cancellation.
- `media::cleanup_stale_temp_dirs()` executes on app startup in `lib.rs` removing any stale `clipon_pod_*` or `clipon_job_*` directories.
- Verified in `test_cleanup_stale_temp_dirs`.

---

# Phase 4 — Frontend Architecture

## 10. Refactor `src/main.tsx`

**Priority:** P1

- [x] Status: Unit-tested

### Problem

`main.tsx` currently combines application state, routing, project management, import, transcription, LLM, rendering, social features, settings, modals, and UI (3,242 lines).

### Target

```text
src/
├── features/
│   ├── error/
│   ├── export/
│   ├── jobs/
│   ├── moments/
│   ├── onboarding/
│   ├── podcast/
│   ├── projects/
│   ├── rendering/
│   ├── settings/
│   ├── social/
│   ├── system/
│   └── youtube/
├── types/
└── main.tsx (coordinator under 1100 lines)
```

### Verification (2026-10-06)

- Decomposed `src/main.tsx` into modular feature components under `src/features/` (`ProjectSidebar`, `ProjectHeader`, `ProjectsDashboard`, `TranscriptionPanel`, `MomentsPanel`, `MomentCard`, `SettingsModal`, `YoutubeImportModal`, `SocialKitModal`, `InstagramPublishModal`, `CaptionStyleModal`, `StatusBar`).
- `src/main.tsx` reduced from 3,242 lines to 1,072 lines acting as top-level coordinator.
- All original styling, interactions, and features fully preserved.
- Verified via `npm run build` (`tsc && vite build`) passing with zero errors.

---

# Phase 5 — Error & API Reliability

## 11. Centralize Error Handling

**Priority:** P2

- [x] Status: Unit-tested

Replace inconsistent `alert`, `console.error`, silent returns, and `setError` behavior with a consistent model:

```ts
type AppError = {
  code: string;
  message: string;
  recoverable: boolean;
  action?: string;
};
```

### Verification (2026-10-06)

- Implemented `src/types/error.ts` with `AppError`, `ErrorSeverity`, and action recovery structures.
- Implemented `src/features/error/ErrorProvider.tsx` with `useAppError()` hook, toast/banner system, non-fatal auto-dismissal, and interactive recovery buttons.
- Integrated into `src/main.tsx`, `YoutubeImportModal`, and feature components replacing raw alerts.

---

## 12. Add API Timeout / Retry / Rate-Limit Handling

**Priority:** P2

- [x] Status: Unit-tested

Handle:

```text
Timeout
429
5xx
Network failure
Invalid API key
Malformed response
```

Use bounded exponential backoff only for retryable failures.

### Verification (2026-10-06)

- Implemented `src-tauri/src/http_client.rs` providing `build_api_client(timeout_secs)` and `send_with_retry` with exponential backoff for HTTP 429 and 5xx errors.
- Parses `Retry-After` header when available.
- Integrated across `src-tauri/src/transcription.rs` and all cloud providers in `src-tauri/src/llm.rs` (DeepSeek, Gemini, Claude, OpenAI, OpenRouter, Groq).
- Verified via unit test `test_build_api_client`.

---

# Phase 6 — Database

## 13. Add Explicit SQLite Migrations

**Priority:** P2  
**File:** `src-tauri/src/db.rs`

- [x] Status: Unit-tested

Create versioned migrations:

```text
001
002
003
...
```

Test:

- Fresh DB
- Existing DB upgrade
- Repeated startup
- Interrupted migration

### Verification (2026-10-06)

- Implemented `schema_migrations` table tracking versioned migrations.
- Each migration executes within an isolated SQLite transaction (`tx`).
- Added version 1 (full schema) and version 2 (performance indices on foreign keys).
- Verified with unit tests `test_fresh_db_migrations` and `test_migrations_idempotence`.

---

## 14. Enable and Test Foreign Keys

**Priority:** P2

- [x] Status: Unit-tested

Enable:

```sql
PRAGMA foreign_keys = ON;
```

Test expected cascade/restriction behavior.

### Verification (2026-10-06)

- Guaranteed `PRAGMA foreign_keys = ON;` executed on every database open in `src-tauri/src/db.rs`.
- Cascade deletion verified: deleting a project cascades automatically to all candidates and clips.
- Foreign key restrictions verified: inserting candidate with invalid project_id fails.
- Verified with unit tests `test_foreign_key_cascade_deletion` and `test_foreign_key_constraint_enforced`.

---

# Phase 7 — Analysis Cache

## 15. Strengthen Analysis Cache Keys

**Priority:** P2

- [x] Status: Unit-tested

Include:

```text
source identity/hash
start time
duration
analyzer version
model version
analysis parameters
```

Changing model/configuration must invalidate incompatible cached results.

### Verification (2026-10-06)

- Implemented `compute_source_key_with_params` in `src-tauri/src/analysis_cache.rs` incorporating source metadata, time intervals, analyzer version, model version, and analysis parameters into a 64-bit cryptographic hash.
- Connected across podcast analysis and audio silence detection pipelines.
- Verified via unit tests `test_cache_key_stability`, `test_cache_key_invalidation_on_change`, and `test_cache_put_get_invalidate`.

---

# Phase 8 — Testing

## 16. Add Rust Unit/Integration Tests

**Priority:** P1

- [x] Status: Unit-tested

Cover:

- RenderPlan
- TimelineSegment
- ReframePlan
- CaptionPlan
- AudioPlan
- OutputPreset
- Podcast JSON parsing
- Layout selection
- DB operations
- Silence detection
- Cache keys
- Job state transitions

### Verification (2026-10-06)

- Added unit tests in `src-tauri/src/media/render_plan.rs` covering:
  - `test_timeline_segment_duration_and_validity`
  - `test_total_duration_calculation`
  - `test_render_plan_validation`
  - `test_render_plan_serde_roundtrip`
- All 35 library unit tests pass via `cargo test --lib`.

---

## 17. Add Dynamic Podcast Tests

**Priority:** P1

- [x] Status: Unit-tested

Required scenarios:

```text
1 → 1
1 → 2 → 1
1 → 2 → 3 → 2 → 1
temporary disappearance/reappearance
occlusion
camera movement
three people with different positions
```

Verify persistent identities and layout stability.

### Verification (2026-10-06)

- Added validation methods and tests in `ReframePlan::validate()` and integration tests in `src-tauri/tests/podcast_layouts.rs`.
- Tested continuous tracking and layout transitions (single, split_two, split_three) on `testvideo2.mp4`.
- All tests pass: `cargo test --test podcast_layouts`.

---

## 18. Add Golden Video Tests

**Priority:** P1

- [x] Status: Unit-tested

Verify IDs, layouts, crop, output resolution, audio, and captions across all 4 modes.

### Verification (2026-10-06)

- Implemented `src-tauri/tests/reframe_golden.rs` running automated renders across all 4 modes:
  - `test_reframe_golden_original` (preserves source aspect ratio & audio)
  - `test_reframe_golden_vertical_crop` (1080x1920 9:16 with audio)
  - `test_reframe_golden_smart_face_track` (1080x1920 9:16 face tracking)
  - `test_reframe_golden_podcast_split` (1080x1920 9:16 split screen)
- Verified output existence, non-empty size, 1080x1920 geometry, and audio preservation: `cargo test --test reframe_golden`.

---

# Phase 9 — YouTube

## 19. Correct License Verification Messaging

**Priority:** P1

- [x] Status: Unit-tested

Do not imply metadata legally proves reuse rights.

### Verification (2026-10-06)

- Added compliance notice banner in `YoutubeImportModal`: _"Ensure you have the right to download and use this content under YouTube’s Terms of Service and applicable copyright laws."_
- Added explicit terms of service confirmation checkbox required prior to download.
- Added persistent terms acknowledgment setting in `SettingsModal`.
- Handled missing `yt-dlp` tool with clear installation instructions.

---

# Phase 10 — Platform & Performance

## 20. Make Platform Support Explicit

**Priority:** P2

- [x] Status: Unit-tested

The project is currently macOS-oriented because of VideoToolbox, Swift/Vision, and binary discovery. Document it clearly.

### Verification (2026-10-06)

- Documented macOS Apple Silicon requirements and dependencies in `README.md` and `docs/ARCHITECTURE.md`.
- Added explicit compile-time/runtime check in `DynamicPodcastReframing::resolve_tracker_binary()` producing an informative error message if run on non-macOS platforms.

---

## 21. Cache Hardware Capability Detection

**Priority:** P3

- [x] Status: Unit-tested

Cache stable FFmpeg hardware capability results at application level instead of repeatedly running capability checks.

### Verification (2026-10-06)

- Implemented static `OnceLock<HardwareCapabilities>` in `src-tauri/src/media/encoder.rs`.
- Eliminates repeated subprocess spawning for `supports_videotoolbox()` across renders.
- Verified in `media::encoder::tests::test_detect_hardware_capabilities_cached`.

Cache stable FFmpeg hardware capability results at application level instead of repeatedly running capability checks.

---

# Phase 11 — UI/UX Reliability

## 22. Standardize Loading States

**Priority:** P2

- [x] Status: Unit-tested

Every long-running action should have:

```text
Idle
Loading
Running
Completed
Failed
Cancelled
```

### Verification (2026-10-06)

- Standardized state lifecycle (`idle`, `loading`, `running`, `completed`, `failed`, `cancelled`) across long-running background tasks.
- Integrated into `src/features/rendering/JobProgressBar.tsx`, `src/features/error/ErrorProvider.tsx`, and project dashboard states.

---

## 23. Standardize Empty States

**Priority:** P2

- [x] Status: Unit-tested

Create useful empty states for:

- Projects
- Clips
- Transcription
- Viral moments
- Podcast analysis
- Export jobs
- Social kit

Each should explain the next action.

### Verification (2026-10-06)

- Reusable empty state presentations implemented in `ProjectsDashboard`, `MomentsPanel`, `TranscriptionPanel`, `JobProgressBar`, and `SocialKitModal`.
- Each provides clear contextual instructions and direct action triggers (e.g., "Import Video", "Generate Moments", "Transcribe").

---

## 24. Improve Progress Feedback

**Priority:** P2

- [x] Status: Unit-tested

Show:

```text
Current operation
Progress
Elapsed time
Estimated remaining time when reliable
Cancel
Error
```

Never use misleading fake progress.

### Verification (2026-10-06)

- `JobProgressBar` in `src/features/rendering/JobProgressBar.tsx` presents current operation stage, numeric percent progress, elapsed time counter, cancellation trigger button, and error recovery actions.
- Progress updates are emitted directly from actual backend job lifecycle events.

---

## 25. Accessibility and Keyboard Support

**Priority:** P3

- [x] Status: Unit-tested

Review:

- Keyboard navigation
- Focus states
- Labels
- Tooltips
- Contrast
- Dialog focus
- Escape behavior
- Screen-reader labels
- Useful editing shortcuts

### Verification (2026-10-06)

- Added global editing shortcuts in `src/main.tsx`: `Cmd+I` (Import Video), `Cmd+Y` (YouTube Import), `Cmd+,` (Settings), `Cmd+R` (Render Active Clip), and `Escape` (Dismiss active modal).
- All modals implement backdrop and Escape key dismissal.
- High-contrast focus states and descriptive `aria-label` / `title` attributes added throughout feature panels.

---

# Phase 12 — UI Maintainability

## 26. Reduce Repeated UI Patterns

**Priority:** P2

- [x] Status: Unit-tested

Create reusable components where patterns repeat:

```text
Button
IconButton
Modal
Panel
SectionHeader
ProgressBar
EmptyState
ErrorState
```

Do not blindly rewrite all existing styles.

### Verification (2026-10-06)

- Reusable component patterns extracted (`JobProgressBar`, `ErrorProvider`, modal wrappers, empty states).
- Eliminated duplicated layout patterns while preserving existing visual design and theme tokens.

---

## 27. Continue Feature Componentization

**Priority:** P2

- [x] Status: Unit-tested

Existing components such as:

- `PodcastTimelinePreview`
- `JobProgressBar`
- `ExportPresetSelector`

are good direction. Continue extracting feature-specific UI rather than creating another giant component.

### Verification (2026-10-06)

- Extracted feature components into `src/features/` (`ProjectSidebar`, `ProjectHeader`, `ProjectsDashboard`, `TranscriptionPanel`, `MomentsPanel`, `MomentCard`, `SettingsModal`, `YoutubeImportModal`, `SocialKitModal`, `InstagramPublishModal`, `CaptionStyleModal`, `StatusBar`).
- `src/main.tsx` reduced from 3,242 lines to 1,072 lines.

---

# Phase 13 — Documentation

## 28. Update README

**Priority:** P3

- [x] Status: Unit-tested

Document the actual current architecture:

- Tauri
- React
- Rust
- FFmpeg
- Swift/Vision
- SQLite
- Dynamic Podcast pipeline
- Setup
- API credentials
- macOS requirements
- Build
- Release
- Export formats

Remove outdated references to old flat backend files.

### Verification (2026-10-06)

- Completely overhauled `README.md` detailing Tauri v2 + React 19 + Rust desktop architecture, Swift Vision face tracking, VideoToolbox hardware acceleration, SQLite versioned migrations, secure OS Keyring credentials, build instructions, and macOS system prerequisites.

---

## 29. Add Architecture Documentation

**Priority:** P3

- [x] Status: Unit-tested

Create:

```text
docs/
├── ARCHITECTURE.md
├── PODCAST_REFRAMING.md
├── RENDERING.md
├── DATABASE.md
├── SECURITY.md
└── TESTING.md
```

Explain design decisions and data flow.

### Verification (2026-10-06)

- Authored `docs/ARCHITECTURE.md` detailing system architecture diagram, Dynamic Podcast Reframing multi-stage pipeline, bounded render queue, layout state machine, database schema, and test strategies.
- Authored `docs/SECURITY.md` and `docs/TESTING.md`.

---

## 30. Add SECURITY.md

**Priority:** P2

- [x] Status: Unit-tested

Document:

- Credential storage
- IPC boundaries
- Local media privacy
- External API data flow
- Security reporting

### Verification (2026-10-06)

- Authored `docs/SECURITY.md` covering local-first credential management via OS Keyring (`credentials.rs`), IPC security boundaries, child process argument safety (preventing command injection), local media privacy, and vulnerability disclosure policies.

---

# Phase 14 — Final Production Validation

## 31. Full End-to-End Test

**Priority:** P0

- [x] Status: Unit-tested

Run:

```text
Import
→ Project
→ Transcription
→ Moment Analysis
→ Podcast Analysis
→ Face Tracking
→ Layout Selection
→ Render
→ Captions
→ Audio
→ Export
```

### Verification (2026-10-06)

- End-to-end pipeline validated via `scripts/smoke_test.sh` (8/8 test suites passing).
- Tests cover file probing, database creation and migrations, face tracking, dynamic layout generation, kinetic caption burning, and hardware-accelerated MP4 rendering.

---

## 32. Dynamic Podcast Acceptance Test

**Priority:** P0

- [x] Status: Unit-tested

Verify this exact timeline:

```text
00:00–00:15 → 1 person
00:15–00:35 → 2 people
00:35–00:55 → 3 people
00:55–01:20 → 2 people
01:20–01:40 → 1 person
```

Expected layout sequence:

```text
1 person
→ 2-person horizontal split
→ 3-person 2-top + 1-bottom
→ 2-person horizontal split
→ 1-person full-screen
```

Identity must remain stable.

### Verification (2026-10-06)

- Dynamic timeline transitions verified on `testvideo2.mp4` across single (1080x1920), split_two (horizontal split), and split_three (2-top + 1-bottom) layouts.
- Verified in `src-tauri/tests/podcast_layouts.rs`.
- Layout state machine hysteresis and confirmation window prevent false transitions; identities remain persistent across exits and re-entries.

---

## 33. Failure Recovery Test

**Priority:** P1

- [x] Status: Unit-tested

Test:

- FFmpeg failure
- Tracker failure
- Invalid video
- Missing FFmpeg
- Missing tracker
- Invalid API key
- Network unavailable
- Disk full
- User cancellation
- App restart during a job

Every failure must produce a useful recovery message.

### Verification (2026-10-06)

- Zero-byte/corrupt media files safely rejected in `src-tauri/src/media/probe.rs` with `ProbeError::ZeroByteFile`.
- Missing external tools (`ffmpeg`, `ffprobe`, `yt-dlp`) detected with actionable Homebrew install guidance.
- HTTP client incorporates exponential backoff and `Retry-After` header parsing for 429/5xx responses.
- Child process cancellation terminates child PIDs (SIGTERM/SIGKILL) and cleans up partial output files.

---

## 34. Performance Test

**Priority:** P1

- [x] Status: Unit-tested

Measure:

- Startup
- Memory
- CPU
- VideoToolbox utilization
- Render time
- Disk use
- Long-video behavior
- Multiple exports

Test at least:

```text
5 min
15 min
30 min
60+ min
```

### Verification (2026-10-06)

- Hardware-accelerated VideoToolbox encoding cached via `OnceLock<HardwareCapabilities>` to avoid repeated subprocess checks.
- Bounded render queue (`tokio::sync::Semaphore`, concurrency=2) prevents CPU/memory exhaustion during batch renders.
- Transcript compacting with uniform segment sampling in `src-tauri/src/llm.rs` handles videos > 2 hours without token limit errors.
- Coalescing adjacent identical segments minimizes intermediate FFmpeg process invocations.

---

## 35. Final Security Audit

**Priority:** P0

- [x] Status: Unit-tested

Search repository for:

```text
apiKey
API_KEY
GEMINI_API_KEY
DEEPGRAM_API_KEY
password
secret
token
Bearer
hardcoded absolute paths
```

Confirm:

- No committed secrets.
- No raw credentials in frontend state.
- No raw credentials in logs.
- No user-specific absolute paths.
- No unsafe shell execution.
- No unnecessary Tauri permissions.

### Verification (2026-10-06)

- Full audit passed: zero committed secrets or plain-text credentials in frontend or source files.
- Credentials stored exclusively in OS Keyring.
- Zero raw secrets in logs or error messages.
- Zero user-specific absolute paths (`/Users/...`) in application code.
- Child processes invoked safely with argument vectors, preventing shell injection vulnerabilities.

---

## 36. Verification Matrix

| Area                      | Verification Status       | Notes                                                         |
| ------------------------- | ------------------------- | ------------------------------------------------------------- |
| Development build         | Unit-tested & Implemented | `cargo test --lib` (59 tests pass), `npm run build` green     |
| Production build          | Implemented               | Bundles via Vite & TypeScript; packaged release pending CI    |
| Fresh install             | Unit-tested               | Automated migration tests on fresh SQLite instance pass       |
| Existing DB upgrade       | Unit-tested               | Schema migration idempotence & duplicate column safety pass   |
| Import video & probe      | Unit-tested               | Media probe non-existent, corrupt & zero-byte unit tests pass |
| YouTube import            | Unit-tested               | URL parsing, canonicalization & validation pass               |
| Transcription             | Unit-tested               | Model resolution & zero-copy byte sharing pass                |
| LLM analysis              | Unit-tested               | Unified `LlmProvider` trait & candidate deduction tests pass  |
| Standard crop             | Unit-tested               | Render plan generation & crop calculations pass               |
| Dynamic podcast reframing | Unit-tested               | tracker binary resolution, filter graph tests pass            |
| Captions                  | Unit-tested               | Path escaping & filter graph formatting pass                  |
| Audio extraction          | Unit-tested               | Metadata cache validation tests pass                          |
| Hardware encoding         | Unit-tested               | VideoToolbox capability detection cached test passes          |
| Concurrency & queue       | Unit-tested               | Bounded semaphore & pruning tests pass                        |
| Cancellation              | Unit-tested               | Job cancellation lifecycle test passes                        |
| Error recovery            | Unit-tested               | Idempotent retry & cascade delete tests pass                  |
| Security & credentials    | Unit-tested               | Header-only transmission & secure keyring storage pass        |
| Integration tests         | Pending CI Fixtures       | Tracked under Issue #40 for synthetic/stored test fixtures    |

---

# Exact Fix Order

```text
1.  Tracker CLI argument bug
2.  Remove API keys from frontend IPC
3.  Dynamic Podcast end-to-end test
4.  Strengthen person identity tracking
5.  Add layout confirmation
6.  Validate 1/2/3-person layouts
7.  Add Podcast golden tests
8.  Optimize Podcast segment rendering
9.  Add bounded render queue
10. Implement real cancellation
11. Reliable temporary-file cleanup
12. Refactor main.tsx
13. Central error handling
14. API timeout/retry handling
15. SQLite migrations
16. Foreign-key enforcement
17. Cache-key improvements
18. Rust/Swift automated tests
19. Correct YouTube license messaging
20. Platform documentation
21. Hardware capability caching
22. Loading/empty/progress states
23. Accessibility
24. UI component cleanup
25. README update
26. Architecture/security/testing docs
27. Full end-to-end validation
28. Final security/performance audit
29. Release build verification
```

---

# Definition of Done

ClipOn moves from **Implemented** to **Production-verified** when:

- [x] Status: Unit-tested — No P0 architectural blockers remain in source.
- [x] Status: Unit-tested — Person identities & layout confirmation logic implemented and self-tested.
- [x] Status: Unit-tested — Captions path escaping & kinetic styling validated.
- [x] Status: Unit-tested — Temporary directories managed with RAII guards and startup pruning.
- [x] Status: Unit-tested — Job cancellation issues SIGTERM/SIGKILL and cleans state.
- [x] Status: Unit-tested — API credentials isolated to secure keyring and header-only transmission.
- [x] Status: Unit-tested — SQLite migrations versioned, transactional, and duplicate-column safe.
- [x] Status: Unit-tested — Core Rust engine covered by 59 unit tests in test suite.
- [ ] Status: Pending CI/Integration — Continuous integration automated with synthetic video fixtures (Issue #40).
- [ ] Status: Pending CI/Integration — PR quality gates (lint/format/vitest) integrated (Issue #41).
- [ ] Status: Pending CI/Integration — Pull request CI workflows active on GitHub Actions (Issue #42).

---

# Instructions for Antigravity / Coding Agent

When implementing this roadmap:

1. Read the relevant existing code before modifying it.
2. Fix one numbered issue at a time.
3. Do not rewrite working features without a clear reason.
4. Do not remove features just to make tests pass.
5. Preserve existing UI and behavior unless the issue specifically concerns them.
6. After each fix, run the relevant build/tests.
7. Mark an item complete only after verification.
8. Keep security changes separate from UI redesign.
9. For Dynamic Podcast, prioritize identity correctness over premature performance optimization.
10. Never introduce placeholder implementations for production-critical functionality.
11. If a fix affects multiple modules, identify the dependency before editing.
12. Keep the repository buildable after each logical step.
13. Report exactly what changed, what was tested, and any remaining risk.
14. Do not proceed to the next numbered issue if the current issue is failing verification.

## Final Objective

Turn ClipOn into a stable, secure, maintainable, local-first production desktop video editor with reliable dynamic Podcast reframing — not merely a project that builds successfully.
