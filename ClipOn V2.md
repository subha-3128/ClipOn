# CLIPON V2 — COMPLETE ARCHITECTURE + DYNAMIC PODCAST ENGINE

## Project

**App:** ClipOn  
**Repository:** https://github.com/subha-3128/ClipOn.git  
**Goal:** Upgrade the existing ClipOn codebase into a robust, local-first long-form-to-shorts/reels editor while preserving all existing functionality.

---

# 1. CORE INSTRUCTION

Work directly inside the existing ClipOn repository.

Do **not** create a separate demo application, mock implementation, prototype, or parallel architecture.

Do **not** remove existing working features unless they are being replaced by a strictly better implementation that preserves the same user-facing behavior.

First inspect the entire repository and understand the current architecture. Then implement the improvements below in the existing codebase.

The final result must build and run successfully.

---

# 2. PRESERVE EXISTING FEATURES

All currently supported functionality must continue working:

- Center Crop
- Original 16:9
- Podcast Split Screen
- Dead Air Cut
- Studio Audio
- Punch Zoom
- Captions
- Transcription
- Local Whisper
- Deepgram transcription/diarization
- YouTube import
- Viral moment ranking
- Social publishing kit
- Instagram publishing
- Project management
- SQLite persistence
- Apple VideoToolbox encoding
- Existing Smart Face Track functionality
- Existing Shorts/Reels workflows

Do not break these while refactoring.

---

# 3. PHASE 0 — COMPLETE REPOSITORY AUDIT

Before modifying code:

1. Inspect all frontend files.
2. Inspect all Rust/Tauri files.
3. Inspect the Swift face tracker.
4. Inspect FFmpeg/media processing.
5. Inspect transcription and diarization.
6. Inspect SQLite/database models.
7. Inspect project state management.
8. Inspect Tauri commands and capabilities.
9. Inspect environment/API credential handling.
10. Inspect hardcoded paths and developer-specific values.
11. Inspect existing tests/build scripts.
12. Identify duplicated or tightly coupled logic.

Create a concise internal architecture map before implementation.

Do not stop after the audit.

---

# 4. PHASE 1 — SECURITY AND CREDENTIALS

Remove unnecessary exposure of API keys and access tokens to the React frontend.

Currently sensitive values may include:

- Deepgram API key
- Gemini API key
- OpenAI API key
- Instagram access token
- Other publishing credentials

Requirements:

- Keep secrets in the Rust/backend side whenever possible.
- Prefer macOS Keychain or an appropriate secure credential store.
- Frontend should receive only safe status information such as:
  - `has_deepgram_key`
  - `has_gemini_key`
  - `has_openai_key`
  - `has_instagram_token`
- Do not send raw secrets to the frontend.
- Do not store sensitive tokens in `localStorage`.
- Audit Tauri commands so secrets cannot accidentally be returned.
- Remove unnecessary credential logging.

The application must still function with the required services.

---

# 5. PHASE 2 — REMOVE HARDCODED PATHS

Find and remove all developer-specific paths such as:

- `/Users/subhajitbepari/...`
- Desktop-specific paths
- Absolute project paths
- Temporary developer directories

Use:

- App data directory
- Project directory
- User-selected paths
- Tauri path APIs
- Configurable storage locations

The app must work on another Mac without editing source code.

---

# 6. PHASE 3 — MEDIA ARCHITECTURE REFACTOR

The existing media processing code is too large and tightly coupled.

Refactor it into logical modules where appropriate.

Suggested structure:

```text
media/
├── mod.rs
├── probe.rs
├── ffmpeg.rs
├── filters.rs
├── renderer.rs
├── encoder.rs
├── captions.rs
├── audio.rs
├── podcast.rs
└── presets.rs
```

Adapt the structure to the existing project instead of blindly creating files.

Responsibilities should be separated:

- Media probing
- FFmpeg command construction
- Video filters
- Audio processing
- Caption rendering
- Podcast rendering
- Encoding
- Export presets

Avoid one giant media-processing function.

---

# 7. PHASE 4 — INTRODUCE A RENDER PLAN

Create a central render abstraction.

Conceptually:

```rust
struct RenderPlan {
    source: String,
    timeline: Vec<TimelineSegment>,
    reframe: ReframePlan,
    captions: Option<CaptionPlan>,
    audio: AudioPlan,
    output: OutputPreset,
}
```

Adapt names/types to the existing codebase.

The RenderPlan should describe:

- Source media
- Selected timeline
- Cropping/reframing
- Podcast layouts
- Captions
- Audio processing
- Encoding/output settings

Rendering should consume this plan instead of mixing analysis, decisions, and FFmpeg execution together.

---

# 8. PHASE 5 — JOB SYSTEM

Introduce a proper processing job model.

Suggested states:

```text
Queued
Analyzing
Processing
Encoding
Completed
Failed
Cancelled
```

Every long-running operation should have:

- Job ID
- Project ID
- Current state
- Progress
- Current stage
- Error information
- Start time
- Completion time

Frontend should receive structured progress events.

Example:

```text
Analyzing video       20%
Tracking people       45%
Building layout       60%
Rendering             78%
Encoding              94%
Completed             100%
```

Avoid blocking the UI during processing.

---

# 9. PHASE 6 — CANCELLATION

Implement real cancellation.

The user should be able to cancel:

- FFmpeg
- Whisper
- Deepgram-related processing
- Face tracking
- YouTube downloads
- Long-running analysis

Maintain a job → child-process relationship so cancellation can terminate the correct process.

Cancellation must leave the project in a clean state.

---

# 10. PHASE 7 — ANALYSIS CACHE

Avoid repeating expensive analysis.

Create a cache system for information such as:

```text
analysis/
├── media.json
├── transcript.json
├── speakers.json
├── faces.json
├── people.json
├── layouts.json
└── silence.json
```

Cache should be invalidated when necessary.

Cache keys should consider:

- Source media identity/hash
- File modification time where appropriate
- Analysis settings
- Tracker version
- Transcription configuration
- Relevant model/version

If only the render settings change, do not rerun expensive face tracking or transcription unnecessarily.

---

# 11. PHASE 8 — ROBUST MULTI-PERSON PODCAST ENGINE

This is the most important part of ClipOn V2.

The Podcast feature must support:

- 1 person
- 2 people
- 3 people
- People entering
- People leaving
- Temporary occlusion
- People moving
- People changing position
- Speaker changes
- Silent participants remaining visible

Do NOT determine the number of people or layout from the first frame.

The system must continuously analyze the selected source video.

---

# 12. DENSE FACE/PERSON TRACKING

The existing tracker samples too sparsely.

Replace the current simplistic approach with continuous/dense analysis.

Target approximately:

```text
3–5 analysis samples per second
```

or another rate that provides reliable tracking while remaining performant.

Do not require every source frame to be analyzed if interpolation/tracking between frames is reliable.

The tracker should identify:

- Face/person bounding box
- Center
- Size
- Confidence
- Visibility
- Temporal continuity

Conceptual model:

```rust
struct TrackedPersonKeyframe {
    t: f64,
    person_id: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    confidence: f64,
    visible: bool,
}
```

---

# 13. PERSISTENT PERSON IDENTITIES

Use persistent identities:

```text
Person 1
Person 2
Person 3
...
```

Identity must NOT be based only on:

- Left/right position
- Current speaker
- First detected face
- Current frame ordering

The tracker should use temporal association based on appropriate available signals such as:

- Bounding-box position
- Motion
- Size
- Face geometry
- Landmarks
- Appearance/features when available
- Historical track information
- Confidence

The same physical person must retain the same Person ID.

Avoid:

- Identity swaps
- Duplicate IDs for one person
- Replacing a silent person with the current speaker
- Reordering identities simply because people move

---

# 14. TEMPORARY DISAPPEARANCE HANDLING

A person disappearing for a few frames must not immediately create a new identity.

Use track states such as:

```text
Visible
TemporarilyLost
Reappeared
Exited
```

If a person is temporarily:

- Occluded
- Behind another person
- Outside a crop
- Missed by detection

keep their identity alive for a configurable grace period.

When they return, reconnect them to the existing identity if confidence is sufficient.

Only create a new identity when the system is confident it is a genuinely new person.

---

# 15. LAYOUT HYSTERESIS

Do not change layout because of one missed detection.

Use confidence and temporal persistence.

For example:

- Person enters → require several confident detections before changing layout.
- Person disappears → require a sustained absence before removing them.
- Temporary tracking failure → preserve current layout.

Make thresholds configurable.

Avoid visual flickering between layouts.

---

# 16. PODCAST LAYOUT MODEL

Use an explicit layout model.

Conceptually:

```rust
enum PodcastLayout {
    Single {
        person_id: usize,
    },

    Two {
        top_person: usize,
        bottom_person: usize,
    },

    Three {
        top_left: usize,
        top_right: usize,
        bottom: usize,
    },
}
```

Adapt this to the actual architecture.

---

# 17. ONE-PERSON LAYOUT

When only one person is confidently present:

```text
┌─────────────┐
│             │
│   PERSON 1  │
│             │
│             │
└─────────────┘
```

Output:

```text
9:16
```

Use dynamic face tracking.

Do not use a fixed crop position.

---

# 18. TWO-PERSON LAYOUT

When two people are present:

```text
┌─────────────┐
│   PERSON 1  │
├─────────────┤
│   PERSON 2  │
└─────────────┘
```

Person 1 = top.

Person 2 = bottom.

Each section should receive approximately half of the vertical output.

Maintain dynamic face-centered crops.

---

# 19. THREE-PERSON LAYOUT

When three people are present, DO NOT create three equal horizontal rows.

Use:

```text
┌─────────┬─────────┐
│ PERSON1 │ PERSON2 │
├─────────┴─────────┤
│      PERSON 3     │
└───────────────────┘
```

Required mapping:

- Person 1 → top-left
- Person 2 → top-right
- Person 3 → bottom full-width

All three sections must represent the exact same source timestamp.

---

# 20. DYNAMIC LAYOUT EXAMPLE

The system should be able to generate timelines such as:

```text
00:00–00:15 → 1 person
00:15–00:35 → 2 people
00:35–00:55 → 3 people
00:55–01:20 → 2 people
01:20–01:40 → 1 person
```

The actual timeline must come from analysis, not hardcoded values.

---

# 21. EXACT TIMESTAMP SYNCHRONIZATION

This is critical.

If the source timestamp is:

```text
00:37.500
```

then every layout region must represent:

```text
00:37.500
```

from the same source frame/time.

Never create independent timelines for:

- Person 1
- Person 2
- Person 3

The layout changes the crop/window, not the source time.

---

# 22. SPEAKER DIARIZATION VS VISUAL IDENTITY

Keep these concepts separate.

Visual tracking determines:

```text
Person 1
Person 2
Person 3
```

Diarization determines:

```text
Speaker S1
Speaker S2
Speaker S3
```

They may be mapped:

```text
S1 → Person 2
S2 → Person 1
S3 → Person 3
```

but speaker identity must never replace visual identity.

A silent participant must remain visible.

---

# 23. SPEAKER/PERSON MAPPING

Where diarization is available:

1. Detect speakers.
2. Detect visual people.
3. Temporally align the two.
4. Build an optional mapping.
5. Use the mapping for:
   - Captions
   - Speaker labels
   - Analytics
   - Optional emphasis

Do not use speech activity to remove silent people.

---

# 24. LOCAL WHISPER LIMITATION

Treat local Whisper primarily as transcription unless separate diarization exists.

Do not falsely claim that Whisper identifies multiple people if the current local implementation only produces one speaker stream.

Deepgram diarization may provide speaker labels when configured.

The architecture must support both.

---

# 25. DYNAMIC CROPPING

Every person crop must follow the person dynamically.

The crop should adapt to:

- Face position
- Person movement
- Bounding-box size
- Headroom
- Frame boundaries

Use interpolation between tracking keyframes.

Avoid:

- Static center crop
- Average crop position
- First-frame crop for the entire video

---

# 26. SMOOTH CAMERA MOVEMENT

Do not make the crop jump every time the detection moves.

Apply smoothing/interpolation.

Use appropriate:

- Temporal interpolation
- Easing
- Position smoothing
- Size smoothing

The result should look like a professional camera operator following the participant.

---

# 27. SEGMENT-BASED RENDERING

Prefer rendering based on layout segments rather than generating one enormous FFmpeg filter expression.

Example:

```text
Segment 1 → Single
Segment 2 → Two
Segment 3 → Three
Segment 4 → Two
Segment 5 → Single
```

Render each segment using the same source timeline.

Then concatenate the resulting segments without changing playback time.

This should make dynamic layouts easier to debug and maintain.

---

# 28. CAPTIONS

Captions must remain synchronized with the source timeline.

When the layout changes:

- Captions must not reset.
- Caption timing must remain accurate.
- Captions must remain inside safe areas.
- Caption positioning should adapt when necessary.
- Speaker-aware captions may use diarization if available.

Do not create separate caption timelines for each person.

---

# 29. DEAD AIR CUT

Dead Air Cut must remain independent from visual tracking.

If Dead Air Cut modifies the timeline:

1. Create the final trimmed timeline first.
2. Apply visual analysis/rendering against the correct timeline.
3. Preserve synchronization.

Do not allow silence removal to desynchronize captions or person tracking.

---

# 30. PUNCH ZOOM

Punch Zoom must remain compatible with:

- Single person
- Two people
- Three people
- Dynamic layout segments
- Face tracking

Do not let Punch Zoom replace the underlying person identity system.

It should operate as an additional render effect.

---

# 31. STUDIO AUDIO

Studio Audio must remain independent of visual layout.

Keep audio processing modular.

It must continue working with:

- Podcast layouts
- Captions
- Punch Zoom
- Dead Air Cut
- Existing export modes

---

# 32. VIDEO ENCODING

Continue supporting Apple Silicon hardware acceleration where available.

Detect hardware capability rather than assuming it exists.

Conceptually:

```rust
struct HardwareCapabilities {
    videotoolbox: bool,
    h264: bool,
    hevc: bool,
}
```

Use:

```text
h264_videotoolbox
```

when supported.

Provide a software FFmpeg fallback when hardware encoding is unavailable.

Do not make the application fail simply because VideoToolbox is unavailable.

---

# 33. EXPORT PRESETS

Create clean export presets:

```text
Instagram Reels
YouTube Shorts
TikTok
Custom
```

All vertical presets should support:

```text
1080 × 1920
9:16
```

Allow custom output configuration where appropriate.

---

# 34. ANALYSIS PREVIEW

Add a Podcast analysis preview/timeline.

The user should be able to see:

```text
00:00–00:15  Person 1
00:15–00:35  Person 1 + Person 2
00:35–00:55  Person 1 + Person 2 + Person 3
...
```

Show:

- Person IDs
- Layout
- Timeline
- Confidence where useful
- Speaker mapping where available

This helps users verify analysis before expensive rendering.

---

# 35. MANUAL CORRECTIONS

Provide a way to correct tracking mistakes.

Support where practical:

- Lock Person ID
- Reassign detection
- Merge tracks
- Split tracks
- Override layout
- Override segment
- Manually set person crop
- Ignore false detection

Manual corrections should be stored as project data and applied during rendering.

---

# 36. FRONTEND ARCHITECTURE

The current `main.tsx` is too large.

Refactor into feature-based modules.

Suggested structure:

```text
src/
├── features/
│   ├── projects/
│   ├── studio/
│   ├── podcast/
│   ├── transcription/
│   ├── moments/
│   ├── publishing/
│   └── settings/
├── components/
├── hooks/
├── lib/
├── types/
└── main.tsx
```

Adapt to the existing project.

`main.tsx` should primarily bootstrap the application.

Do not create unnecessary abstraction for its own sake.

---

# 37. PROJECT STATE

Persist relevant analysis/render state through the existing SQLite architecture.

Project state should support:

- Source media
- Analysis status
- Transcript
- Speaker information
- Person tracks
- Layout segments
- Manual corrections
- Render settings
- Export history
- Job status

Do not duplicate the same state unnecessarily between SQLite, React state, and files.

---

# 38. STRUCTURED ERRORS

Replace unclear string-only errors with structured errors where practical.

Example:

```text
Code
Message
Stage
Recoverable
Details
```

Frontend should display useful user-facing messages.

Do not expose secrets or internal stack traces unnecessarily.

---

# 39. PERFORMANCE

The application is local-first.

Optimize for Apple Silicon.

Important rules:

- Do not repeatedly decode the entire video unnecessarily.
- Cache expensive analysis.
- Reuse analysis results.
- Avoid unnecessary React re-renders.
- Avoid blocking the UI.
- Run CPU-heavy work outside the UI thread.
- Prefer efficient frame sampling.
- Reuse FFmpeg/media processes where appropriate.
- Clean temporary files safely.

---

# 40. TESTING

Create tests for the critical logic.

At minimum cover:

### Person tracking

- One person
- Two people
- Three people
- Person enters
- Person leaves
- Temporary occlusion
- Person returns
- People crossing
- People moving
- Silent participant
- Speaker change
- Identity persistence

### Layout

- Single
- Two
- Three
- 3-person 2-top + 1-bottom
- Dynamic transitions
- No flickering

### Rendering

- Exact timestamp synchronization
- Dynamic crop
- Caption synchronization
- Punch Zoom compatibility
- Dead Air Cut compatibility
- Studio Audio compatibility
- Hardware encoding
- Software fallback

### Existing features

Verify existing workflows still build and work.

---

# 41. NO IDENTITY SWAPPING

This is a hard requirement.

Example:

```text
Person 1 enters at 00:00.
Person 2 enters at 00:20.
Person 1 moves to the right.
Person 2 moves to the left.
```

The system must still know:

```text
Person 1 = original Person 1
Person 2 = original Person 2
```

Do not rename them based on screen position.

---

# 42. NO SPEAKER-BASED REPLACEMENT

Example:

```text
Person 1 is speaking.
Person 2 is silent.
```

Output must still show both if both are visually present.

Then:

```text
Person 2 starts speaking.
```

Person 1 must not disappear merely because Person 2 is now the speaker.

---

# 43. LAYOUT STABILITY

Avoid rapid layout changes such as:

```text
1 → 2 → 1 → 2 → 1
```

caused by detection noise.

Use:

- Confidence thresholds
- Temporal persistence
- Grace periods
- Track state
- Hysteresis
- Smoothing

The final video should feel intentional and professional.

---

# 44. SAFETY AGAINST FALSE DETECTIONS

Do not count every face/person detection as a real participant.

Use:

- Minimum confidence
- Minimum persistence
- Reasonable size thresholds
- Temporal consistency
- Track validation

False detections should not trigger a new layout.

---

# 45. CLEAN ARCHITECTURE RULE

Do not create a system where:

```text
React
  ↓
FFmpeg command string
  ↓
Everything
```

Instead use:

```text
UI
 ↓
Project/Render Settings
 ↓
Analysis
 ↓
RenderPlan
 ↓
Renderer
 ↓
FFmpeg
 ↓
Output
```

For Podcast:

```text
Source Video
 ↓
Frame Analysis
 ↓
Person Tracks
 ↓
Speaker Mapping
 ↓
Layout Timeline
 ↓
Manual Corrections
 ↓
RenderPlan
 ↓
Segment Renderer
 ↓
FFmpeg
 ↓
Final 9:16 Video
```

---

# 46. DO NOT OVERENGINEER

Use the existing dependencies where they are sufficient.

Do not introduce heavy new frameworks without a real reason.

Prefer:

- Existing Rust crates
- Existing Swift tooling
- Existing FFmpeg
- Existing SQLite
- Existing React architecture

Add dependencies only when they solve a concrete problem.

---

# 47. CODE QUALITY

Follow these rules:

- No dead code.
- No unused imports.
- No debug secrets.
- No developer-specific paths.
- No fake implementations.
- No placeholder functions for core functionality.
- No TODOs pretending to complete required features.
- No duplicated business logic.
- Clear function names.
- Small focused modules.
- Strong typing where useful.
- Useful comments only where logic is non-obvious.

---

# 48. BUILD AND VERIFY

After implementation:

1. Run frontend type checking.
2. Run frontend build.
3. Run Rust formatting.
4. Run Rust checks.
5. Run Rust tests.
6. Build Tauri.
7. Verify Swift tracker compilation if applicable.
8. Verify FFmpeg commands.
9. Verify existing features.
10. Verify Podcast workflows.

Fix all errors introduced by the refactor.

Do not stop at the first successful compile if important workflows remain broken.

---

# 49. FINAL ACCEPTANCE CRITERIA

ClipOn V2 is complete only when:

- Existing features still work.
- No API secrets are unnecessarily exposed to React.
- Sensitive tokens are not stored in localStorage.
- No hardcoded developer paths remain.
- Media processing is modularized.
- RenderPlan exists and is actually used.
- Long-running work uses a job system.
- Jobs can be cancelled.
- Expensive analysis is cached.
- Podcast supports 1 person.
- Podcast supports 2 people.
- Podcast supports 3 people.
- Three-person layout is:
  - Person 1 top-left
  - Person 2 top-right
  - Person 3 bottom full-width
- People can enter and leave dynamically.
- Temporary disappearance does not immediately change identity/layout.
- Person IDs remain persistent.
- Identity does not depend on speaker or left/right position.
- Silent participants remain visible.
- Dynamic face tracking works throughout the video.
- Crops smoothly follow people.
- Layout changes are stable.
- All layout sections use the same source timestamp.
- Captions remain synchronized.
- Speaker diarization is separate from visual identity.
- Dead Air Cut remains compatible.
- Punch Zoom remains compatible.
- Studio Audio remains compatible.
- VideoToolbox is used when available.
- Software encoding fallback works.
- Manual tracking/layout corrections are supported where practical.
- Podcast analysis can be previewed before rendering.
- Frontend is modularized.
- Project state is persisted correctly.
- Structured errors and progress reporting work.
- Tests cover critical tracking/layout/rendering cases.
- The application builds successfully.

---

# 50. IMPLEMENTATION ORDER

Follow this order:

```text
PHASE 0  → Repository audit
PHASE 1  → Credential/security cleanup
PHASE 2  → Remove hardcoded paths
PHASE 3  → Media architecture refactor
PHASE 4  → RenderPlan
PHASE 5  → Job system
PHASE 6  → Cancellation
PHASE 7  → Analysis cache
PHASE 8  → Multi-person Podcast engine
PHASE 9  → Dynamic layout renderer
PHASE 10 → Podcast preview/timeline
PHASE 11 → Manual corrections
PHASE 12 → Frontend modularization
PHASE 13 → Export presets
PHASE 14 → Testing and regression verification
```

Do not skip directly to UI changes before the underlying analysis/rendering architecture is correct.

---

# FINAL COMMAND

START by inspecting the existing ClipOn repository.

Then implement the changes in the order above.

Do not only describe the architecture.

Do not only create a plan.

Do not create a mock.

Do not create a separate application.

Actually modify and improve the existing ClipOn codebase.

Preserve all existing functionality while implementing the new ClipOn V2 architecture and robust dynamic multi-person Podcast engine.

At the end, build and test the application and report:

1. What was changed.
2. What files/modules were added or modified.
3. What existing features were preserved.
4. What tests were run.
5. Any remaining limitations.
