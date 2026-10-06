I completed a static, repository-wide audit of `subha-3128/ClipOn` using the GitHub connector. I did **not modify the repository**.

## Overall assessment

**Current engineering quality: ~7/10 as a development-stage desktop app, but not production-ready yet.**

The project has a surprisingly solid foundation: the frontend has been modularized, Rust owns most heavy processing, SQLite migrations and foreign keys exist, the render pipeline has a real `RenderPlan`, there is a bounded job manager, cancellation support, temp-directory cleanup, API retry handling, and a genuine multi-person podcast architecture.

However, there are several issues that I would treat as **production blockers**, especially around secrets, Instagram publishing, export presets, AI scoring, and some reliability guarantees.

The biggest concern is that the repository's own documentation/roadmap says several items are “completed and verified,” while the current implementation still contains gaps in those areas. That creates a dangerous false sense of completion.

---

# 1. CRITICAL — Credential save can silently delete existing API keys

### What is the problem?

The Settings screen keeps API-key input fields initialized to empty strings:

`src/main.tsx:108-115`

```ts
const [deepgramKey, setDeepgramKey] = useState("");
const [anthropicKey, setAnthropicKey] = useState("");
const [deepseekKey, setDeepseekKey] = useState("");
const [geminiKey, setGeminiKey] = useState("");
const [openaiKey, setOpenaiKey] = useState("");
const [openrouterKey, setOpenrouterKey] = useState("");
const [groqKey, setGroqKey] = useState("");
```

When Settings is saved:

`src/main.tsx:781-795`

```ts
await Promise.all([
  ["deepgram", deepgramKey],
  ["gemini", geminiKey],
  ...
].map(([name, value]) => invoke("save_credential", { name, value })));
```

But the backend interprets an empty value as **delete this credential**:

`src-tauri/src/lib.rs:1806-1823`

```rust
if value.trim().is_empty() {
    credentials::delete(&name)
}
```

### Why it matters

A user can already have a valid Gemini/DeepSeek/OpenAI/etc. key stored in the keychain, open Settings, change an unrelated option, and press Save.

Because untouched fields are empty, the other stored credentials can be deleted.

This is a real data-loss bug.

### Where

- `src/main.tsx:108-115`
- `src/main.tsx:781-795`
- `src-tauri/src/lib.rs:1806-1823`
- `src/features/settings/SettingsModal.tsx:198-355`

### How to fix

Use explicit credential operations:

```text
None      → leave existing key untouched
Some(key) → replace key
Some("")  → intentionally delete key
```

Better:

```ts
saveCredential("gemini", {
  changed: true,
  value: newKey,
});
```

Only send credentials whose value actually changed.

Also never treat an empty controlled input as “delete” unless the user explicitly clicked a Delete/Clear button.

### Priority

**CRITICAL**

---

# 2. CRITICAL — Instagram access token is stored in plaintext and localStorage

### What is the problem?

The code stores the Instagram access token in browser local storage:

`src/main.tsx:394`

```ts
localStorage.setItem(
  "clipon_instagram_access_token",
  metaModalAccessToken.trim()
);
```

The backend also writes the Instagram token directly into `.env`:

`src-tauri/src/lib.rs:1755-1800`

```rust
std::env::set_var("INSTAGRAM_ACCESS_TOKEN", token);
...
lines.push(format!("INSTAGRAM_ACCESS_TOKEN={token}"));
```

### Why it matters

This directly contradicts the project's own `docs/SECURITY.md`, which says credentials are supposed to remain in the OS keyring.

A plaintext `.env` in application data is much weaker than Keychain/Credential Manager/Secret Service.

`localStorage` is also inappropriate for long-lived access tokens.

### Where

- `src/main.tsx:394`
- `src-tauri/src/lib.rs:1755-1800`
- `src-tauri/src/lib.rs:73`
- `src/features/settings/SettingsModal.tsx:346-355`
- `src/features/social/InstagramPublishModal.tsx`

### How to fix

Treat Meta credentials exactly like Deepgram/Gemini/etc.:

```text
React
  ↓
save_instagram_credential
  ↓
OS keychain
  ↓
Rust Instagram service
```

Frontend should know only:

```text
has_instagram_token: boolean
instagram_account_id: string
```

Do not put the access token into:

- localStorage
- SQLite
- `.env`
- process environment
- URLs

### Priority

**CRITICAL**

---

# 3. CRITICAL — Instagram publishing uploads private videos to anonymous third-party hosts

### What is the problem?

For one Instagram API path, the application uploads the local video first to:

- `uguu.se`
- `tmpfiles.org`
- `catbox.moe`

before giving the URL to Meta.

`src-tauri/src/instagram.rs:64-174`

### Why it matters

This is one of the biggest architectural/security problems in ClipOn.

A user's podcast/interview/recording is leaving their machine and being copied to third-party file-hosting services without an explicit upload-consent flow.

This also undermines the product's **“local-first”** claim.

Even if those services are temporary, you have:

- loss of control over retention
- third-party privacy risk
- accidental public exposure
- dependency on unrelated services
- potential availability failures
- increased legal/privacy responsibility

### Where

`src-tauri/src/instagram.rs:64-174`

### How to fix

Prefer an official upload flow supported by the selected Meta API.

If a public HTTPS video URL is genuinely required:

```text
Local file
 ↓
controlled temporary storage
 ↓
short-lived signed URL
 ↓
Meta ingestion
 ↓
automatic deletion
```

At minimum:

- explicit user consent
- encrypted storage
- short TTL
- audit logging
- deletion confirmation

### Priority

**CRITICAL**

---

# 4. HIGH — Instagram tokens are also leaked through URLs

### What is the problem?

Several Instagram Graph API requests put the access token in query parameters.

For example:

`src-tauri/src/instagram.rs:241,249,307,358,415,441,511,591`

```text
...?access_token=...
```

### Why it matters

Credentials in URLs are much easier to leak through:

- logs
- proxy telemetry
- HTTP diagnostics
- crash reports
- request tracing
- debugging tools

### How to fix

Use authentication headers wherever supported.

Do not build URLs like:

```text
...?access_token=SECRET
```

and ensure errors never stringify the full request URL.

### Priority

**HIGH**

---

# 5. HIGH — Export preset selector does not actually control rendering

### What is the problem?

The frontend provides:

- Instagram Reels
- YouTube Shorts
- TikTok

in `ExportPresetSelector.tsx`.

But the backend render path always creates:

`src-tauri/src/media/renderer.rs:583-596`

```rust
OutputPreset::instagram_reels()
```

So regardless of what the user chooses, rendering defaults to the Instagram preset.

### Why it matters

The UI is presenting a feature that is effectively cosmetic.

For example:

```text
User selects YouTube Shorts
        ↓
UI says 60fps / 8 Mbps
        ↓
Backend renders Instagram preset
```

That is a correctness bug.

### Where

- `src/features/export/ExportPresetSelector.tsx`
- `src/features/moments/MomentsPanel.tsx:272`
- `src-tauri/src/media/renderer.rs:583-596`
- `src-tauri/src/media/presets.rs`

### How to fix

Pass the selected preset through Tauri:

```text
frontend preset
    ↓
render_flat_clip_for_candidate(...)
    ↓
RenderPlan
    ↓
OutputPreset
```

Do not hardcode `instagram_reels()` inside the generic renderer.

### Priority

**HIGH**

---

# 6. HIGH — The multi-modal viral score formula is mathematically wrong

### What is the problem?

`src-tauri/src/pro_editor.rs:284-286`

```rust
let composite =
    (0.40 * draft.score + 0.35 * audio_score + 0.25 * pacing_score).round();
```

But:

```text
draft.score      = 0.0–1.0
audio_score      = ~45–99
pacing_score     = ~50–98
```

So the supposedly 40% LLM contribution contributes at most:

```text
0.40 × 1.0 = 0.4
```

while audio contributes roughly:

```text
0.35 × 90 = 31.5
```

### Why it matters

The comments say:

> 40% LLM + 35% Audio + 25% Speech

but the implementation does not do that.

The resulting ranking is dominated almost entirely by audio and pacing.

### How to fix

Normalize everything first:

```rust
let llm_score = draft.score.clamp(0.0, 1.0);
let audio_score = (audio_score / 100.0).clamp(0.0, 1.0);
let pacing_score = (pacing_score / 100.0).clamp(0.0, 1.0);

let composite =
    100.0 * (
        0.40 * llm_score +
        0.35 * audio_score +
        0.25 * pacing_score
    );
```

Then validate ranking with real examples.

### Priority

**HIGH**

---

# 7. HIGH — Heuristic social-kit generation can panic on Unicode

### What is the problem?

`src-tauri/src/llm.rs` truncates the transcript using a byte index:

```rust
&transcript_text[..140]
```

### Why it matters

Rust strings are UTF-8.

If byte 140 lands in the middle of an Indian-language character, emoji, accented character, etc., this can panic.

This matters particularly because ClipOn is intended for real-world multilingual recordings.

### How to fix

Use Unicode-safe truncation:

```rust
transcript_text.chars().take(140).collect::<String>()
```

or a proper grapheme-aware truncator if desired.

### Priority

**HIGH**

---

# 8. HIGH — Manual podcast layout override is presented but not wired

### What is the problem?

`PodcastTimelinePreview` defines:

```ts
onLayoutOverride?: (...)
```

and exposes:

```text
Dynamic (AI)
1-Person
2-Split
3-Split
```

But `MomentsPanel` renders the component without supplying that callback:

`src/features/moments/MomentsPanel.tsx:...`

```tsx
<JobProgressBar ... />
```

and its podcast preview usage supplies no usable override callback.

### Why it matters

The UI tells the user:

> Manual Correction & Layout Lock

but the action does not actually modify the render pipeline.

That is a misleading control.

### How to fix

Make the override part of the candidate/render state:

```text
candidate.layoutOverride
        ↓
RenderPlan.reframe
        ↓
PodcastSplit { layout_override }
        ↓
renderer
```

Persist it per candidate.

### Priority

**HIGH**

---

# 9. HIGH — YouTube compliance is enforced only in the frontend

### What is the problem?

`YoutubeImportModal.tsx` performs the copyright check first:

`src/features/youtube/YoutubeImportModal.tsx:34-68`

But the actual Tauri command can be invoked independently.

The backend download operation does not receive proof that the frontend's ToS acknowledgement/license gate occurred.

### Why it matters

Frontend controls are not security/compliance boundaries.

Another UI path or future refactor could invoke:

```text
download_youtube_video
```

without the same checks.

### Additional issue

The checkbox is persisted globally in `localStorage`:

`src/features/youtube/YoutubeImportModal.tsx:22-31`

So a user can acknowledge once and remain permanently “acknowledged.”

### How to fix

Move the final validation into the backend:

```text
download request
 ↓
validate URL
 ↓
validate policy/acknowledgement
 ↓
perform metadata check
 ↓
download
```

Treat UI validation as convenience, not enforcement.

### Priority

**HIGH**

---

# 10. HIGH — YouTube URL input is not sufficiently constrained

### What is the problem?

The application passes the user-provided URL directly to `yt-dlp`.

Because `yt-dlp` supports many extractors, this is broader than “download a YouTube video.”

### Why it matters

ClipOn's feature is advertised as YouTube import, but the backend potentially delegates to a general extractor.

That creates:

- unexpected network access
- unsupported sites being downloaded
- confusing UX
- larger attack surface

### How to fix

Parse the URL and allow only:

```text
youtube.com
www.youtube.com
youtu.be
```

Then canonicalize to a video ID before invoking `yt-dlp`.3

### Priority

**HIGH**

---

# 11. HIGH — Re-running AI moment detection destroys previous candidate state

### What is the problem?

`src-tauri/src/db.rs:307-359`

```rust
DELETE FROM candidates WHERE project_id = ?1
```

then it inserts an entirely new candidate set.

Because clips and Instagram posts reference candidates with `ON DELETE CASCADE`, rerunning analysis can destroy associated clip/post history.

### Why it matters

A user could:

1. generate moments
2. render several clips
3. publish one
4. tweak their prompt/model
5. regenerate moments

and lose the historical relationships.

### How to fix

Prefer immutable analysis runs:

```text
projects
  └── candidate_runs
        └── candidates
             └── clips
```

Keep prior runs unless the user explicitly deletes them.

At minimum, don't cascade-delete published history.

### Priority

**HIGH**

---

# 12. HIGH — Auto-render on Instagram publish can lose clip association

### What is the problem?

`publish_candidate_to_instagram()` finds the clip first:

`src-tauri/src/lib.rs:1869-1873`

Then, if none exists, it calls the render function:

`src-tauri/src/lib.rs:1875-1891`

The local `clip` variable is **not refreshed afterward**.

Later:

`src-tauri/src/lib.rs:1925-1932`

```rust
clip.as_ref().map(|c| c.id.as_str())
```

can therefore still be `None`.

### Why it matters

The newly rendered clip can exist while the Instagram post record still has no `clip_id`.

That weakens project history and UI linking.

### Fix

After automatic rendering:

```text
render
 ↓
query created clip
 ↓
use its ID when creating instagram_posts
```

Better still, make rendering return a structured result containing:

```text
clip_id
output_path
job_id
status
```

### Priority

**HIGH**

---

# 13. HIGH — Social publishing ignores the user's selected LLM provider

### What is the problem?

`src-tauri/src/llm.rs:1034-1060`

```rust
generate_social_kit()
```

always tries:

1. Gemini
2. DeepSeek
3. heuristic generation

regardless of the user's selected `llmEngine`.

### Why it matters

A user choosing:

```text
OpenAI
Claude
Groq
OpenRouter
Local
```

can still trigger Gemini/DeepSeek network calls.

That can cause:

- unexpected cost
- unexpected data transmission
- privacy violations
- confusing configuration behavior

### Fix

Use one centralized provider router:

```text
selected provider
       ↓
provider adapter
       ↓
generate social kit
```

Only use fallback providers when the user explicitly enables fallback.

### Priority

**HIGH**

---

# 14. HIGH — Long transcripts are sampled by dropping most of the conversation

### What is the problem?

`src-tauri/src/llm.rs:627-648`

For transcripts above 1200 segments:

```rust
segments.iter().step_by(step)
```

### Why it matters

This discards most transcript content.

A key viral moment between sampled segments can disappear completely.

This is especially problematic for long:

- podcasts
- interviews
- lectures
- streams

### Fix

Use temporal chunking:

```text
Transcript
 ↓
30–60 min chunks
 ↓
candidate extraction per chunk
 ↓
candidate deduplication
 ↓
global ranking
```

Or use a hierarchical summarization / map-reduce pipeline.

### Priority

**HIGH**

---

# 15. HIGH — AI candidate timestamps are too permissively validated

### What is the problem?

`src-tauri/src/llm.rs:650-735`

Invalid values frequently become:

```rust
0.0
```

instead of being rejected.

The parser also does not fully enforce:

- `start >= 0`
- `end > start`
- `end <= transcript.duration`
- non-overlapping candidates
- meaningful duration bounds

### Why it matters

A malformed model response can become a real render job.

The renderer then attempts to process invalid timing.

### Fix

Treat model output as untrusted input.

Validate every candidate:

```text
finite
start >= 0
end > start
end <= duration
duration within configured range
hook non-empty
score within range
```

Reject invalid entries instead of silently converting them.

### Priority

**HIGH**

---

# 16. HIGH — Podcast preview can trigger expensive face analysis repeatedly

### What is the problem?

`PodcastTimelinePreview.tsx` automatically calls:

```ts
get_podcast_preview(...)
```

inside `useEffect`.

There can be multiple candidate previews in the interface, so opening a project can initiate multiple separate tracker analyses.

### Why it matters

Face tracking is one of the most expensive operations in ClipOn.

Repeatedly analyzing overlapping source ranges can produce:

- CPU/GPU load
- Swift process spawning
- UI lag
- duplicate cache lookups
- unnecessary disk activity

### Fix

Use a shared analysis service/cache:

```text
source + start + duration
        ↓
shared cache
        ↓
all candidate previews
```

Also consider only analyzing:

- the selected candidate
- the expanded candidate
- on explicit Preview click

rather than every mounted card.

### Priority

**HIGH**

---

# 17. MEDIUM/HIGH — Rendered output path can collide between projects

### What is the problem?

`src-tauri/src/lib.rs:1293-1315`

Output slug is based primarily on the source filename.

For example:

```text
interview.mp4
interview.mp4
```

from different projects can produce the same output directory.

### Why it matters

Two projects can write into the same location.

That can lead to:

- overwriting
- confusing history
- wrong clips being displayed
- race conditions

### Fix

Use:

```text
<slug>-<project-id-short>
```

or persist a unique output directory ID when the project is created.

### Priority

**HIGH**

---

# 18. MEDIUM/HIGH — Deleting a project does not remove its stored project files

### What is the problem?

`src-tauri/src/db.rs:516-520`

```rust
DELETE FROM projects WHERE id = ?1
```

This removes the database row.

It does not remove the project's internal directory.

### Why it matters

You can end up with:

```text
SQLite says project does not exist
but
data_dir/projects/<id>/ still exists
```

leading to storage leaks and orphaned media.

### Fix

Define project ownership explicitly:

```text
DB delete
+
project-directory delete
+
temporary artifact cleanup
```

or keep files intentionally but mark them as orphaned and expose cleanup.

### Priority

**MEDIUM/HIGH**

---

# 19. MEDIUM — “Clear Full Storage” is broader than it should be

### What is the problem?

`src-tauri/src/lib.rs:1992-2025` uses `remove_dir_all()` on several user directories:

```text
~/Documents/ClipOn/Clips
~/Documents/ClipOn/Youtube Video
~/Downloads/ClipOn
```

### Why it matters

If users manually place unrelated files in those folders, ClipOn can delete them too.

Also, “Clear Full Storage” does not truly clear everything because:

- custom directories are not necessarily cleared
- keyring credentials remain
- Instagram `.env` credentials remain
- `AnalysisCache::global()` may use a different location

### Fix

Only delete files ClipOn created and tracks.

Use an ownership manifest or per-project directories.

Make “Delete ClipOn data” and “Delete credentials” separate explicit actions.

### Priority

**MEDIUM/HIGH**

---

# 20. MEDIUM — Analysis cache has inconsistent storage locations

### What is the problem?

`AnalysisCache::global()` uses:

`src-tauri/src/analysis_cache.rs:21-25`

```rust
dirs::data_dir()
    .map(|d| d.join("clipon"))
```

but application state uses Tauri's:

```rust
app.path().app_data_dir()
```

And `clear_all_storage()` creates a cache against `state.data_dir`.

### Why it matters

The cache used by:

- podcast analysis
- silence detection

may not be the same cache directory that “Clear Full Storage” clears.

### Fix

Create exactly one application cache path and inject it into `AppState`.

Avoid multiple definitions of “ClipOn data directory.”

### Priority

**MEDIUM**

---

# 21. MEDIUM — Podcast rendering can spawn a very large number of FFmpeg processes

### What is the problem?

`renderer.rs:188-349` may:

```text
segment 1 → FFmpeg
segment 2 → FFmpeg
segment 3 → FFmpeg
...
concat → FFmpeg
final → FFmpeg
```

The segment-merging optimization is good, but a rapidly changing layout can still create many processes.

### Why it matters

This can become the primary bottleneck for long podcast renders.

### Fix

The current segmented approach should remain as a correctness fallback, but eventually add:

```text
fast path:
single FFmpeg filtergraph for compatible timelines

fallback:
segment render + concat
```

The architecture already hints at this; it just needs to be implemented further.

### Priority

**MEDIUM**

---

# 22. MEDIUM — Frontend batch rendering is still sequential

### What is the problem?

`src/main.tsx` contains:

```ts
for (const candidate of selected) {
   ...
}
```

with rendering awaited sequentially.

### Why it matters

The backend has a semaphore allowing **2 concurrent jobs**, but the frontend effectively feeds it one at a time.

So the queue capacity is underutilized.

### Fix

Submit all selected jobs to the backend queue:

```text
Job 1
Job 2
Job 3
Job 4
...
```

and let the backend enforce the concurrency limit.

### Priority

**MEDIUM**

---

# 23. MEDIUM — JobProgressBar is not scoped to a job

### What is the problem?

`JobProgressBar` supports:

```ts
currentJobId?: string | null
```

but `MomentsPanel` does not pass one:

`src/features/moments/MomentsPanel.tsx:274`

```tsx
<JobProgressBar onJobComplete={...} onJobCancel={...} />
```

The listener therefore accepts every job:

`src/features/jobs/JobProgressBar.tsx:25-28`

```ts
if (!currentJobId || payload.id === currentJobId)
```

### Why it matters

As soon as multiple renders run:

```text
Job A → 20%
Job B → 60%
Job A → 30%
```

the same UI can jump between unrelated jobs.

### Fix

Render a job row per active job, or explicitly bind the component to a job ID.

### Priority

**MEDIUM**

---

# 24. MEDIUM — Completed jobs accumulate indefinitely in memory

### What is the problem?

`JobManager` stores jobs in:

```rust
HashMap<String, JobInfo>
```

but completed/failed/cancelled jobs are never removed.

### Why it matters

A long-lived desktop session can accumulate thousands of job records.

Not catastrophic, but it is a straightforward memory-growth issue.

### Fix

Keep:

```text
active jobs
```

in memory and retain only a bounded recent history:

```text
last 50 / last 100 jobs
```

or persist history in SQLite.

### Priority

**MEDIUM**

---

# 25. MEDIUM — Cancellation immediately sends SIGTERM and SIGKILL

### What is the problem?

`src-tauri/src/jobs.rs:230-234`

```rust
kill -15
kill -9
```

are issued back-to-back.

### Why it matters

The application doesn't give FFmpeg a graceful opportunity to terminate.

That increases the chance of:

- truncated files
- incomplete metadata
- harder cleanup
- unnecessary filesystem churn

### Fix

Use:

```text
SIGTERM
 ↓
short grace period
 ↓
check process
 ↓
SIGKILL only if needed
```

### Priority

**MEDIUM**

---

# 26. MEDIUM — Renderer silently treats probe failure as “no video”

### What is the problem?

`src-tauri/src/media/renderer.rs:115-116`

```rust
let probe = probe_media(&plan.source).ok();
let has_video = probe.as_ref().map(|p| p.has_video).unwrap_or(false);
```

If probing fails, the code doesn't distinguish:

```text
valid audio-only file
```

from:

```text
probe failure
```

### Why it matters

A corrupt/unsupported source could progress further than it should and fail later in a less understandable way.

### Fix

Make probe failure fatal when the render requires video:

```rust
let probe = probe_media(&plan.source)
    .context("failed to probe source media")?;
```

### Priority

**MEDIUM**

---

# 27. MEDIUM — FFmpeg concat manifest is not safely escaped

### What is the problem?

`renderer.rs:252-257`

```rust
list_content.push_str(&format!("file '{}'\n", f.to_string_lossy()));
```

Paths containing `'` can break the concat manifest.

### Why it matters

Real filesystem paths can contain apostrophes.

That means some valid project/source paths can fail only during podcast rendering.

### Fix

Implement correct FFmpeg concat-demuxer escaping or use a safer input strategy.

### Priority

**MEDIUM**

---

# 28. MEDIUM — Analysis cache key is described as cryptographic, but isn't

### What is the problem?

`analysis_cache.rs:55-57` uses:

```rust
std::collections::hash_map::DefaultHasher
```

### Why it matters

This is fine for cache indexing, but it should not be described as a cryptographic hash.

More importantly, the key depends on:

```text
mtime rounded to seconds
+
size
+
path
```

so rapid file replacement with identical size can theoretically reuse stale analysis.

### Fix

Use SHA-256 for stronger content identity when needed, or use more precise file metadata plus a cache manifest.

### Priority

**MEDIUM**

---

# 29. MEDIUM — Local Whisper model configuration is misleading

### What is the problem?

`transcription.rs` accepts a `model_path`, but the local Whisper invocation still uses:

```text
--model base
```

and the Python path also defaults to:

```python
"base"
```

### Why it matters

The configuration interface suggests greater model control than the implementation actually provides.

### Fix

Define a real model abstraction:

```text
tiny
base
small
medium
large
custom
```

and pass the selected model explicitly.

### Priority

**MEDIUM**

---

# 30. MEDIUM — Deepgram loads and clones the entire audio file into RAM

### What is the problem?

`transcription.rs:8-13`

```rust
let bytes = tokio::fs::read(audio_path).await?;
let bytes_clone = bytes.clone();
```

### Why it matters

WAV audio is large.

At 16 kHz mono PCM:

```text
~115 MB/hour
```

and cloning the buffer doubles peak memory before network transmission.

### Fix

Prefer streaming/file-backed request bodies where practical, or at least avoid unnecessary clones.

### Priority

**MEDIUM**

---

# 31. MEDIUM — Audio extraction is repeated unnecessarily

### What is the problem?

`transcribe_project()` calls `extract_audio()` every time.

`src-tauri/src/media/audio.rs:9-30`

always invokes FFmpeg and overwrites:

```text
transcription_audio.wav
```

### Why it matters

Repeated transcription/analysis can redo expensive work that hasn't changed.

### Fix

Cache extracted audio against:

```text
source path
+
source size
+
mtime
+
audio extraction parameters
```

### Priority

**MEDIUM**

---

# 32. MEDIUM — Gemini API key is transmitted in both header and URL

### What is the problem?

`src-tauri/src/llm.rs:168-175` and `900-907` use:

```text
?key=...
```

and:

```text
x-goog-api-key
```

simultaneously.

### Why it matters

The URL representation creates unnecessary secret exposure.

### Fix

Keep the key in the header only.

### Priority

**MEDIUM**

---

# 33. MEDIUM — Hard-coded Gemini model fallback list will age badly

### What is the problem?

`src-tauri/src/llm.rs:154-161`

contains several hard-coded model names and a long fallback sequence.

### Why it matters

Provider model names change.

A failure can cause:

```text
request
→ fail
→ request another model
→ fail
→ request another model
...
```

increasing latency and cost.

### Fix

Centralize model configuration, validate availability, and maintain provider adapters rather than hard-coding fallback chains throughout business logic.

### Priority

**MEDIUM**

---

# 34. MEDIUM — LLM provider implementation is heavily duplicated

### What is the problem?

`src-tauri/src/llm.rs` is roughly 1,000+ lines with repeated:

- prompt construction
- model selection
- HTTP calls
- response parsing
- retry logic
- JSON extraction

### Why it matters

Provider behavior will drift.

A bug fixed in Gemini may remain in OpenAI/DeepSeek/etc.

### Fix

Create:

```text
LlmProvider trait
 ├─ GeminiProvider
 ├─ OpenAIProvider
 ├─ DeepSeekProvider
 ├─ ClaudeProvider
 ├─ GroqProvider
 └─ OpenRouterProvider
```

Then keep candidate-selection/business logic independent from provider-specific HTTP.

### Priority

**MEDIUM**

---

# 35. MEDIUM — Rust backend is still too centralized

### What is the problem?

`src-tauri/src/lib.rs` is still over 2,000 lines and mixes:

- Tauri commands
- orchestration
- storage
- environment management
- Instagram
- Ollama
- rendering
- project management
- credential handling

### Why it matters

It's much better than the original 3,000-line frontend coordinator, but the backend has become the next monolith.

### Fix

Split into application services:

```text
commands/
project_service.rs
render_service.rs
transcription_service.rs
candidate_service.rs
credential_service.rs
instagram_service.rs
environment_service.rs
```

Keep `lib.rs` primarily as wiring.

### Priority

**MEDIUM**

---

# 36. MEDIUM — One SQLite connection is globally mutex-protected

### What is the problem?

`src-tauri/src/db.rs:10-13`

```rust
Arc<Mutex<Connection>>
```

### Why it matters

Every DB operation is serialized through one connection.

For a desktop app this is acceptable initially, but eventually it becomes a contention point for:

- jobs
- project refreshes
- render state
- social publishing
- history

### Fix

At minimum:

- WAL mode
- busy timeout
- explicit transactions

Longer term, use a connection pool if actual contention appears.

### Priority

**MEDIUM**

---

# 37. MEDIUM — Migration compatibility code hides errors

### What is the problem?

`db.rs:148-150`

```rust
let _ = conn.execute("ALTER TABLE ...", []);
```

### Why it matters

All errors are discarded.

That means a real migration problem becomes invisible.

### Fix

Accept only:

```text
duplicate-column
```

as harmless.

Return every other error.

### Priority

**MEDIUM**

---

# 38. MEDIUM — Transcript replacement is not transactional

### What is the problem?

`db.rs:286+`

`save_transcript()` deletes the existing transcript and then inserts a new one.

### Why it matters

If insertion fails after deletion, the project loses its previous transcript.

### Fix

Wrap delete + insert in a SQLite transaction.

### Priority

**MEDIUM**

---

# 39. MEDIUM — Documentation claims exceed actual verification

### What is the problem?

`ClipOn_Problem_Fixing_Roadmap.md` repeatedly says items are:

> completed and verified

including:

- secure credentials
- real job cancellation
- production readiness
- dynamic podcast testing
- loading states
- full testing

But the source code still contains several gaps described above.

### Why it matters

This is not merely documentation quality.

It affects engineering decision-making because future work may assume a problem is already solved.

### Fix

Introduce explicit status:

```text
Implemented
Unit-tested
Integration-tested
CI-tested
Production-verified
```

and never mark an item “verified” simply because the code exists.

### Priority

**MEDIUM**

---

# 40. MEDIUM — Integration tests can silently pass without testing the render pipeline

This is one of the most important testing problems.

`src-tauri/tests/reframe_golden.rs` does:

```rust
let Some(source) = test_video() else {
    eprintln!("SKIPPED ...");
    return;
};
```

and `podcast_layouts.rs` does the same for `testvideo2.mp4`.

Those are not failing tests; they simply return successfully.

The repository tree does not contain the test videos because `.mp4` is ignored.

### Why it matters

Your CI can report:

```text
PASS
```

while running **zero real video rendering tests**.

That is particularly concerning because the smoke script concludes:

```text
All Smoke Tests Passed Successfully! System is Production-Ready.
```

### Fix

For CI:

```text
fixture present → run integration test
fixture absent → FAIL CI
```

or store controlled test fixtures in:

- Git LFS
- external test-artifact storage
- CI-generated synthetic media

### Priority

**HIGH**

---

# 41. MEDIUM — There is no frontend lint/test quality gate

### What is the problem?

`package.json` contains only:

```text
dev
build
tauri
tauri:dev
tauri:build
```

There is no:

```text
lint
test
format
```

### Why it matters

TypeScript compilation catches type errors, but not:

- dead code
- inconsistent hooks
- accessibility issues
- common React bugs
- code style drift
- maintainability problems

### Fix

Add:

```text
eslint
prettier
vitest
```

and run them in CI.

### Priority

**MEDIUM**

---

# 42. MEDIUM — No PR CI; release workflow is tag-only

### What is the problem?

`.github/workflows/release.yml` runs only on:

```yaml
push:
  tags:
    - "v*"
```

### Why it matters

A broken change can merge without automatic:

- frontend build
- Rust tests
- integration tests
- security checks

### Fix

Create:

```text
ci.yml
  frontend
  rust
  integration
  clippy

release.yml
  packaging/signing
```

### Priority

**MEDIUM**

---

# 43. MEDIUM — Tauri CSP is disabled

### What is the problem?

`src-tauri/tauri.conf.json`:

```json
"security": {
  "csp": null
}
```

### Why it matters

Desktop WebViews are lower risk than a public website, but ClipOn still handles:

- network APIs
- user-controlled content
- external URLs
- third-party publishing

A restrictive CSP is still valuable defense-in-depth.

### Fix

Define a minimal CSP based on actual resources.

### Priority

**MEDIUM**

---

# 44. MEDIUM — Ollama installation path is strongly macOS-specific

### What is the problem?

`src-tauri/src/lib.rs:300-340` uses:

```text
open -a Ollama
/Applications
Homebrew macOS paths
Ollama-darwin.zip
```

Yet the release pipeline targets:

```text
macOS
Ubuntu
Windows
```

### Why it matters

The UI can advertise a feature that simply doesn't work on other platforms.

### Fix

Expose capabilities explicitly:

```text
platform
local_whisper_supported
ollama_supported
dynamic_podcast_supported
hardware_encoder_supported
```

Then hide/disable unsupported installation paths.

### Priority

**HIGH**

---

# 45. MEDIUM — Cross-platform release packaging is incomplete

The release matrix builds:

```text
macos-latest
ubuntu-22.04
windows-latest
```

but the bundled:

```text
bin/clipon-face-tracker
```

is an Apple Vision/Swift-oriented helper.

### Why it matters

The application cannot honestly provide identical capabilities across all three targets.

### Fix

Either:

```text
macOS → Vision tracker
Windows/Linux → disable dynamic tracker
```

or provide platform-specific tracker implementations/resources.

Also make CI verify the packaged application on each platform.

### Priority

**HIGH**

---

# UI/UX Audit

There is a lot that is already good here, but these areas still deserve work.

## 46. MEDIUM — Styling is too fragmented between CSS and inline styles

`src/styles.css` is ~3,236 lines and contains roughly 200 hard-coded color literals.

Several components also use extensive inline styling:

- `ExportPresetSelector`
- `PodcastTimelinePreview`
- `InstagramPublishModal`
- `JobProgressBar`
- `YoutubeImportModal`

### Why it matters

It becomes difficult to maintain:

- spacing
- colors
- hover states
- responsive behavior
- theme changes

### Fix

Move component styling into a shared design-token layer:

```css
--bg
--surface
--border
--accent
--text
--muted
--danger
--success
```

and reduce inline styling.

### Priority

**MEDIUM**

---

## 47. LOW — Podcast labels still imply “2-person” behavior

Examples:

`ProjectSidebar.tsx`

```text
2-Face
```

and podcast UI language still frequently emphasizes two-person split even though the backend supports 1/2/3 people.

### Why it matters

This makes the feature appear less capable than it is.

### Fix

Use language such as:

```text
Dynamic Podcast
1–3 Person Reframe
```

### Priority

**LOW**

---

## 48. LOW — “Dense 3.5 fps Tracking” conflicts with architecture documentation

`PodcastTimelinePreview.tsx` displays:

```text
Dense 3.5 fps Tracking
```

while `docs/ARCHITECTURE.md` discusses 5 fps.

### Why it matters

Small inconsistency, but it reduces trust in technical documentation.

### Fix

Use a single configured tracking rate and expose it consistently.

### Priority

**LOW**

---

## 49. LOW — Mobile CSS is largely irrelevant to the actual native window constraints

Tauri config sets:

```json
"minWidth": 1040,
"minHeight": 700
```

but `styles.css` contains breakpoints down to:

```text
768px
375px
```

### Why it matters

Not harmful, but it adds complexity for screen sizes the native application currently cannot reach.

### Fix

Either:

- intentionally support small windows later, or
- remove dead responsive code.

### Priority

**LOW**

---

## 50. LOW — Accessibility semantics need another pass

There are many custom modals and buttons, but I did not find a complete accessibility layer such as:

```text
role="dialog"
aria-modal
focus trap
focus restoration
live regions
keyboard navigation
```

The toast system also appears to rely on visual presentation rather than a proper ARIA live region.

### Fix

Add a reusable accessible modal primitive and accessible notification primitive.

### Priority

**LOW/MEDIUM**

---

# Performance review

The biggest real performance risks are:

| Area                                   | Risk                           |
| -------------------------------------- | ------------------------------ |
| Podcast preview analysis per candidate | **High**                       |
| Segment-by-segment FFmpeg rendering    | **High** for complex timelines |
| Deepgram entire WAV in memory + clone  | **Medium/High**                |
| Sequential frontend batch rendering    | **Medium**                     |
| Repeated environment probing           | **Medium**                     |
| Repeated audio extraction              | **Medium**                     |
| Hardware detection                     | **Good — already cached**      |
| Silence detection                      | **Good — cached**              |
| Project filtering                      | **Good — useMemo**             |

One especially good decision is `OnceLock` hardware capability caching in `encoder.rs`; that removes repeated FFmpeg capability probing.

---

# Database/API review

## Good

The database implementation has several strong pieces:

- versioned migrations
- transactional migration application
- `PRAGMA foreign_keys = ON`
- foreign-key cascades
- indexes
- prepared SQL parameters
- central persistence abstraction

Those are exactly the right fundamentals for a local desktop application.

## Needs improvement

The main DB problems are:

```text
candidate regeneration destroys history
single connection serialization
non-transactional transcript replacement
weak status constraints
delete_project leaves files
instagram_posts lacks a hard uniqueness invariant
```

I would not redesign the schema from scratch. The current schema is perfectly usable as the foundation.

---

# Code quality / architecture review

## What is already implemented well

### 1. The frontend modularization is real

The project now has distinct domains:

```text
features/
  error/
  export/
  jobs/
  moments/
  onboarding/
  podcast/
  projects/
  rendering/
  settings/
  social/
  system/
  transcription/
  youtube/
```

That is a substantial improvement over putting everything in one component.

`main.tsx` is still too large, but it is now functioning much more as a coordinator.

### 2. Rust owns the expensive work

This is a good architecture:

```text
React
 ↓
Tauri IPC
 ↓
Rust orchestration
 ↓
FFmpeg / Vision / DB / APIs
```

It is far better than trying to perform media processing in the WebView.

### 3. Command execution avoids shell injection

The code consistently uses:

```rust
Command::new(...)
    .args(...)
```

instead of:

```text
sh -c
bash -c
```

That's an important security positive.

### 4. `RenderPlan` is a good abstraction

`src-tauri/src/media/render_plan.rs` provides a meaningful domain model:

```text
Timeline
ReframePlan
CaptionPlan
AudioPlan
OutputPreset
job_id
```

This is exactly the direction I would continue.

### 5. Job cancellation architecture is conceptually sound

The `JobManager` has:

```text
job state
progress
PID registration
bounded semaphore
cancellation
cleanup
event emission
```

The global manager is shared through cloned `Arc`s, so the renderer and app state are operating over the same underlying job state.

That's a good foundation.

### 6. Temporary render cleanup is well designed

`TempDirGuard` uses RAII:

```rust
impl Drop for TempDirGuard
```

and the startup cleanup strategy is a good defense against interrupted renders.

### 7. Podcast architecture is much better than a fixed two-person implementation

The move toward:

```text
PersonTrack[]
LayoutSegment[]
```

is correct.

The layout state machine/hysteresis idea is also good.

The core structure is something I'd preserve rather than rewrite.

### 8. Error handling has improved substantially

`ErrorProvider` gives the application a shared error model instead of scattering raw alerts everywhere.

### 9. API retry handling exists

The shared HTTP retry layer for transient failures is a good idea.

It is much better than every API function inventing its own retry behavior.

### 10. The media editing utilities are nicely separated

`pro_editor.rs` separates:

```text
boundary snapping
audio scoring
ASS subtitles
dead-air cuts
punch zoom
```

That's a sensible division of responsibilities.

---

# Production-readiness verdict

### Current status

**Not production-ready.**

Not because the architecture is bad.

Rather, because there are a handful of issues that are too important to ship around.

I would classify the current application as:

```text
Architecture:            Good
Feature breadth:         Very good
Code organization:       Good / improving
Media pipeline:          Good foundation
Testing structure:       Good foundation
Security posture:        Needs major correction
Data integrity:          Needs correction
Cross-platform support:  Incomplete
Release engineering:     Incomplete
UX polish:               Good, but inconsistent
```

---

# Recommended improvement roadmap

I would **not** continue adding new features yet.

## Phase 0 — Immediate blockers

### P0-A — Fix credential lifecycle

Implement:

```text
credential unchanged
credential replaced
credential deleted
```

and remove the accidental deletion problem.

### P0-B — Move Instagram credentials completely to Keychain

Remove:

```text
localStorage token
.env token
process.env Instagram token
raw token IPC
```

### P0-C — Remove anonymous video hosting from Instagram publishing

Use an official/controlled media ingestion architecture.

### P0-D — Fix export preset plumbing

Make the selected platform actually reach:

```text
OutputPreset
```

### P0-E — Fix candidate score normalization

Correct the 40/35/25 weighting.

### P0-F — Fix Unicode transcript truncation

Very small change, but it eliminates a real runtime panic.

---

# Phase 1 — Core reliability

### P1-A — Make candidate analysis versioned

Instead of:

```text
DELETE candidates
INSERT candidates
```

use:

```text
analysis_run
  └── candidates
       └── clips
```

### P1-B — Fix project storage ownership

Every project should have:

```text
project UUID
project directory
clip directory
analysis artifacts
```

and deletion should clean only what ClipOn owns.

### P1-C — Unify application data/cache paths

There should be one canonical:

```text
AppPaths
```

service.

### P1-D — Fix JobProgressBar

Support:

```text
Job A
Job B
Job C
```

independently.

### P1-E — Let the frontend submit jobs to the backend queue

Don't serialize the batch in React.

---

# Phase 2 — AI/media reliability

### P2-A — Strict candidate validation

Treat all LLM output as untrusted.

### P2-B — Replace transcript sampling with chunked analysis

For long podcasts:

```text
chunk
→ analyze
→ rank
→ deduplicate
→ global ranking
```

### P2-C — Optimize podcast rendering

Maintain two paths:

```text
Fast single-filtergraph path
Fallback segment-render path
```

### P2-D — Cache audio extraction

Avoid repeated FFmpeg extraction.

### P2-E — Make probe failure explicit

Never silently interpret probe failure as “no video.”

---

# Phase 3 — Architecture cleanup

Refactor the Rust backend from:

```text
lib.rs
```

into:

```text
commands/
services/
providers/
media/
storage/
```

And refactor LLM providers into a common adapter interface.

---

# Phase 4 — Testing / CI

Replace “test skipped = success” with:

```text
missing fixture = failure
```

Then add:

```text
npm lint
npm test
cargo fmt --check
cargo clippy
cargo test
real media integration tests
```

on every PR.

Separate packaging into release-only workflows.

---

# Phase 5 — Release engineering

For actual public distribution:

```text
platform-specific helper binaries
macOS signing
macOS notarization
Windows signing
Linux packaging verification
crash logging
structured logs
versioned migrations
dependency update automation
```

---

# Final priority matrix

| Priority     | Main issues                                                                                                                                                                                                                                                                                                                                                       |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **CRITICAL** | Credential deletion bug, Instagram token storage, third-party video hosting                                                                                                                                                                                                                                                                                       |
| **HIGH**     | Instagram URL secrets, export preset ignored, viral score bug, Unicode panic, podcast override no-op, YouTube backend enforcement, candidate history deletion, Instagram clip association, wrong LLM provider for social kit, long-transcript sampling, candidate validation, podcast preview over-analysis, output-directory collision, cross-platform packaging |
| **MEDIUM**   | Cache path inconsistency, delete-project cleanup, full-storage deletion scope, render process count, job UI scoping, job-history memory, cancellation grace period, probe failures, concat escaping, audio extraction/memory, DB locking/migrations, CSP, duplicated provider logic, backend monolith                                                             |
| **LOW**      | Naming/documentation inconsistencies, responsive dead code, visual polish/accessibility cleanup                                                                                                                                                                                                                                                                   |

## Bottom line

**I would keep the overall architecture. I would not rewrite ClipOn.**

The project already has a good foundation, especially the Rust media layer, `RenderPlan`, job manager, SQLite structure, error system, and dynamic podcast tracking model.

The next step should be a **hardening pass**, not another feature sprint.

The most important work is:

```text
Security
  ↓
Data integrity
  ↓
Rendering correctness
  ↓
AI correctness
  ↓
Testing/CI
  ↓
Architecture cleanup
  ↓
UI polish
```

One important caution: the repository's roadmap currently marks many of these areas as “completed and verified,” but the current source still contains the issues above. I would therefore treat the roadmap as a **historical implementation log**, not as proof that those requirements are actually closed.
