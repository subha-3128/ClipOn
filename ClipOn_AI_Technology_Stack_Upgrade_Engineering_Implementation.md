# ClipOn --- AI Technology Stack Upgrade & Engineering Implementation

**Project:** ClipOn\
**Scope:** AI-assisted clipping for YouTube Shorts and Instagram Reels\
**Approach:** Audit first, improve incrementally, preserve the existing
architecture unless evidence proves a replacement is necessary.

------------------------------------------------------------------------

## 1. Executive Summary

ClipOn is a local-first desktop application that turns long videos into
short-form vertical clips. Its workflow includes transcription,
AI-assisted moment selection, clip-boundary refinement, reframing,
captions, rendering, and publishing support for YouTube Shorts and
Instagram Reels.

The goal of this upgrade is **not to replace the stack or rewrite the
application**. The goal is to make the existing pipeline produce clips
that start quickly, remain understandable without the full source video,
follow the active speaker, preserve subject identity, use stable
framing, and render reliably.

The upgrade should focus on measurable output quality:

-   Strong hooks in the first 1--2 seconds.
-   Accurate start and end boundaries.
-   Self-contained clips with enough context to make sense.
-   Better active-speaker and subject tracking.
-   Stable crops with fewer jumps and identity switches.
-   Readable, well-timed captions.
-   Consistent render quality and predictable performance.
-   A feedback loop that helps improve future candidate ranking.

## 2. Current Architecture to Preserve

The repository currently uses the following broad architecture:

  -----------------------------------------------------------------------
  Area                    Current technology /    Upgrade principle
                          approach                
  ----------------------- ----------------------- -----------------------
  Desktop shell and       Tauri 2 and Rust        Retain; strengthen
  backend                                         service boundaries and
                                                  error handling.

  Frontend                React 19, TypeScript,   Retain; improve
                          Vite                    workflows and expose
                                                  quality signals
                                                  clearly.

  Project persistence     SQLite                  Retain; use migrations
                                                  for any schema changes.

  Media processing        FFmpeg / FFprobe        Retain as the rendering
                                                  and media-inspection
                                                  foundation.

  macOS accelerated       Apple VideoToolbox      Retain with a tested
  encoding                where available         fallback path.

  Face detection and      Apple Vision / Swift    Retain initially;
  tracking                helper on supported     benchmark before
                          macOS paths             considering a
                                                  replacement.

  Transcription           Local Whisper and       Improve reliability,
                          Deepgram options        timing validation, and
                                                  provider fallback
                                                  behavior.

  AI moment ranking       Configurable LLM        Keep provider
                          providers, including    abstraction; add a
                          local and cloud options structured scoring
                                                  layer around model
                                                  output.

  Publishing              YouTube upload and      Retain; improve
                          Instagram publishing    preflight validation
                          integrations            and actionable errors.

  Credentials             OS keyring-backed       Preserve secure
                          secret storage          storage; never write
                                                  API keys to logs or
                                                  ordinary settings
                                                  files.
  -----------------------------------------------------------------------

**Important:** This table describes the broad stack visible in the
project. Before implementation, confirm the current call paths and
provider availability directly in the repository. Do not assume every
optional provider is configured or active.

## 3. Engineering Principles

1.  **Audit before editing.** Trace the actual end-to-end pipeline and
    document where each decision is made.
2.  **Small, reviewable changes.** Avoid broad rewrites, parallel
    duplicate implementations, or speculative dependencies.
3.  **One source of truth.** Use a shared clip-quality model rather than
    unrelated scoring logic scattered across services.
4.  **Structured outputs.** Validate AI responses against explicit
    schemas; treat model output as untrusted input.
5.  **Deterministic fallbacks.** A missing API key, failed model call,
    or unavailable vision capability must not leave a project in an
    ambiguous state.
6.  **Local-first where practical.** Do not require a cloud service for
    operations that the existing local stack can perform.
7.  **Privacy and secret safety.** Keep tokens and API keys out of logs,
    crash reports, prompt traces, and database records.
8.  **Measure before adopting.** New models or dependencies must beat
    the current implementation on a defined benchmark.
9.  **Platform-aware behavior.** Gate Apple-specific functionality by
    platform and capability, and provide a tested fallback where
    required.
10. **No silent degradation.** When quality is reduced because a
    provider or detector failed, explain why in the UI and logs.

## 4. Priority Roadmap

### P0 --- Baseline audit and measurement

Before making changes:

-   Trace the path from video import through transcription, candidate
    generation, editing, reframing, rendering, and publishing.
-   Identify duplicate or competing implementations of clip scoring,
    crop calculation, transcript normalization, and boundary adjustment.
-   Record current behavior for center crop, face tracking, multi-person
    layouts, captions, cancellation, and provider errors.
-   Build a small representative benchmark set covering podcasts,
    interviews, solo speakers, multiple speakers, screen recordings, and
    low-quality footage.
-   Save baseline outputs and record current failures.

**Deliverable:** `docs/AI_PIPELINE_BASELINE.md` with pipeline diagrams,
key file paths, current limitations, and baseline metrics.

### P1 --- Hook-first candidate selection

Improve selection so clips are not ranked only by whether a segment
contains an interesting statement. Give special attention to the
opening.

Assess each candidate for:

-   Hook strength in the first 1--2 seconds.
-   Clarity and specificity.
-   Emotional or informational value.
-   Standalone context.
-   Coherence and completeness.
-   Redundancy with other selected clips.
-   Transcript confidence and timing quality.
-   Whether the clip begins or ends mid-word or mid-thought.

The LLM should return structured candidate data, not an unvalidated
block of prose. Keep the model responsible for semantic judgment, while
deterministic code validates timestamps, duration, transcript alignment,
and required fields.

**Acceptance criteria:**

-   Every candidate has a valid start, end, duration, transcript span,
    and explanation.
-   Invalid or out-of-range timestamps are rejected or repaired through
    a deterministic rule.
-   The ranking output is stable enough to compare in regression tests.
-   Hook quality is measured separately from overall clip quality.

### P2 --- Start and end boundary optimization

Improve clip boundaries using transcript timing and local context.

Implementation sequence:

1.  Normalize transcript segments and word timestamps into a single
    internal representation.
2.  Locate the proposed opening phrase and verify it against the
    transcript.
3.  Remove avoidable dead air, greetings, filler, and weak lead-ins only
    when doing so preserves meaning.
4.  Include the minimum preceding context required to understand a
    reference or answer.
5.  Avoid starting inside a word or cutting off a word at the end.
6.  Preserve complete thoughts and avoid ending before the payoff.
7.  Apply configurable padding around speech boundaries.
8.  Validate the final boundaries against media duration and
    minimum/maximum clip duration.

Do not blindly remove every filler word or greeting. Some are part of
the speaker's meaning, tone, or comedic timing.

**Acceptance criteria:**

-   No clip begins or ends inside a detected word.
-   No clip extends beyond source duration.
-   Boundary adjustments are logged with a reason code.
-   The editor can inspect and manually override the suggested
    boundaries.

### P3 --- Context independence and coherence

A clip should make sense to a viewer who has not watched the original
video.

Evaluate whether the clip:

-   Establishes the topic or question.
-   Explains necessary references such as "he," "that," or "the other
    one."
-   Contains a complete claim, story beat, answer, or payoff.
-   Avoids depending on missing visuals or earlier dialogue.
-   Ends at a natural point.

Use the transcript and nearby context to detect missing setup. If
context cannot be restored within the allowed duration, lower the score
rather than inventing content.

**Acceptance criteria:**

-   Context independence is an explicit score dimension.
-   Candidates that require substantial missing context are ranked
    lower.
-   The system never fabricates transcript content to make a clip appear
    self-contained.

### P4 --- Centralized `ClipQualityScore`

Introduce one shared quality model used by candidate ranking, the
editor, and evaluation tools.

Suggested dimensions, each normalized to a documented range:

  -----------------------------------------------------------------------
  Dimension                           Purpose
  ----------------------------------- -----------------------------------
  `hook_score`                        Strength of the first 1--2 seconds.

  `coherence_score`                   Whether the clip forms a complete,
                                      understandable unit.

  `context_score`                     Whether it makes sense without the
                                      full video.

  `payoff_score`                      Whether the clip reaches a useful
                                      conclusion, reveal, or punchline.

  `speech_quality_score`              Transcript confidence,
                                      intelligibility signals, and timing
                                      quality.

  `visual_quality_score`              Subject visibility, framing, and
                                      visual stability.

  `boundary_score`                    Whether start and end points are
                                      clean.

  `redundancy_penalty`                Reduces near-duplicate candidates.

  `risk_penalty`                      Flags uncertainty, missing context,
                                      or unreliable analysis.
  -----------------------------------------------------------------------

A starting formula can be used as a configurable baseline, not as a
permanent truth:

`ClipQualityScore = weighted positive scores − redundancy penalty − risk penalty`

Weights must be configurable and versioned. Do not treat the model's
self-reported confidence as ground truth. Store the score breakdown so
the UI and benchmark tools can explain why a candidate ranked highly or
poorly.

**Acceptance criteria:**

-   A single shared score schema is used across the pipeline.
-   Scores and weights are versioned.
-   Missing dimensions are handled explicitly, not silently treated as
    perfect scores.
-   Unit tests cover normalization, missing data, invalid values, and
    penalty behavior.

### P5 --- Active-speaker accuracy and identity preservation

Improve subject selection and reframing without immediately replacing
Apple Vision.

Recommended sequence:

1.  Audit current face detection, tracking, and crop-selection call
    paths.
2.  Measure face-detection coverage and identity switches on
    representative videos.
3.  Associate detections across frames using appearance and spatial
    continuity.
4.  Prefer the active speaker when reliable evidence exists; otherwise
    choose a stable, relevant subject.
5.  Avoid switching subjects because of a single missed detection or
    brief occlusion.
6.  Keep tracking state and crop state separate from rendering code.
7.  Preserve a graceful fallback to center crop or the last stable crop
    when tracking confidence is low.

Potential improvements to evaluate after the baseline:

-   **Vision Feature Prints** for appearance-based identity continuity.
-   **Kalman filtering** for smoother position and velocity estimates.
-   **Core ML** only if a measured capability gap justifies another
    model.
-   **SyncDiscriminator or another audio-visual speaker signal** only
    after a small proof of concept demonstrates useful gains on ClipOn's
    target footage.

These are candidates for controlled experiments, not mandatory
dependencies. Avoid adding several tracking systems at once.

**Acceptance criteria:**

-   Fewer identity switches on the benchmark set.
-   Reduced crop jitter without excessive lag.
-   Short occlusions do not cause immediate, arbitrary subject changes.
-   A deterministic fallback is used when active-speaker confidence is
    insufficient.
-   Performance is measured on supported hardware.

### P6 --- Crop smoothing and composition

Make reframing feel intentional rather than mechanically reactive.

-   Smooth crop-center movement over time.
-   Add dead zones or hysteresis so tiny face movements do not move the
    frame.
-   Limit crop velocity and acceleration to avoid sudden jumps.
-   Account for shot changes and genuine speaker changes so smoothing
    does not lag behind the scene.
-   Keep faces and important visual content inside safe margins.
-   Make the crop strategy configurable and visible in the editor.
-   Test portrait, landscape, low-resolution, multi-person, and
    moving-camera footage.
-   Ensure crop calculations use the correct source dimensions,
    rotation, pixel aspect ratio, and output aspect ratio.

**Acceptance criteria:**

-   No invalid crop dimensions or out-of-bounds crop regions.
-   Crop movement is smoother while still responding to meaningful scene
    changes.
-   Important subjects remain visible in benchmark examples.
-   Center-crop fallback remains available.

### P7 --- Caption accuracy and visual quality

Captioning should be synchronized, readable, and consistent with the
clip.

-   Prefer word-level timing when supported by the selected
    transcription engine.
-   Validate subtitle timestamps and clip-relative offsets.
-   Keep text within platform-safe margins.
-   Prevent captions from being clipped at the edges.
-   Avoid excessive words per line and overly rapid caption changes.
-   Handle punctuation, capitalization, numbers, and multilingual text
    carefully.
-   Keep caption rendering separate from transcript acquisition so
    styles can change without retranscribing.
-   Test caption output after the final crop and render, not only in the
    editor preview.

**Acceptance criteria:**

-   Subtitle times are monotonic and within clip duration.
-   Captions do not render outside the safe area.
-   Caption layout is tested at final output resolution.
-   Transcription failure and subtitle-generation failure have separate
    actionable errors.

### P8 --- Visual validation after rendering

Add automated checks for rendered output.

At minimum, validate:

-   File exists and is non-empty.
-   FFprobe can read the output.
-   Duration and resolution are within expected bounds.
-   Audio stream exists when expected.
-   Output aspect ratio matches the selected format.
-   Subtitle timing stays within the rendered duration.
-   The output can be decoded through representative frames.
-   Cancellation does not leave a misleading "completed" state.

For higher-value validation, extract representative frames and inspect
face visibility, crop bounds, black bars, subtitle clipping, and sudden
composition changes. Use automated checks to flag suspicious output,
with manual review for subjective quality.

**Acceptance criteria:**

-   Render success is reported only after validation passes.
-   Failed validation returns a useful error and preserves diagnostic
    details without secrets.
-   The app can distinguish render failure from post-render validation
    failure.

### P9 --- Feedback and ranking improvement

Add an opt-in way to record useful, non-sensitive feedback such as:

-   Candidate kept or rejected.
-   Boundary manually changed.
-   Crop mode changed.
-   Captions edited.
-   Clip exported or published.
-   User's quality rating, if supplied.

Use feedback first to inspect recurring failure patterns and tune
explicit weights. Do not immediately train a custom model from a small
or noisy dataset.

Only evaluate a lightweight ranking model such as LightGBM or XGBoost
after there are enough representative, consistently rated examples. A
practical trigger is roughly **200--500 rated clips**, but data quality
and diversity matter more than the raw count. Compare it against the
rules-based baseline on a held-out set before adoption.

**Acceptance criteria:**

-   Feedback is stored with a documented schema and privacy policy.
-   User feedback can be disabled or deleted.
-   Training/evaluation data excludes credentials and unnecessary
    personal content.
-   Any learned ranker has a reproducible evaluation and rollback path.

## 5. Recommended Technology Decisions

  -----------------------------------------------------------------------
  Technology / capability Decision                Reason
  ----------------------- ----------------------- -----------------------
  Rust + Tauri + React    Keep                    Existing architecture
                                                  supports a native
                                                  desktop app with a
                                                  web-based UI.

  FFmpeg / FFprobe        Keep                    Core media-processing
                                                  and validation
                                                  foundation.

  SQLite                  Keep                    Suitable for local
                                                  project metadata and
                                                  versioned migrations.

  Apple Vision            Keep initially          Existing macOS
                                                  capability; replace
                                                  only if benchmark
                                                  results justify it.

  Whisper / Deepgram      Keep behind a common    Supports local and
                          interface               cloud transcription
                                                  choices.

  Existing LLM providers  Keep behind a provider  Avoid provider lock-in
                          abstraction             and make failure
                                                  handling consistent.

  Central quality scorer  Implement               Makes ranking
                                                  explainable and
                                                  testable across the
                                                  app.

  Vision Feature Prints   Evaluate                May improve identity
                                                  continuity.

  Kalman filter           Evaluate                May reduce jitter in
                                                  tracking and crop
                                                  movement.

  Core ML                 Defer until benchmark   Additional models add
                          evidence                maintenance and
                                                  performance costs.

  SyncDiscriminator /     Prototype only          Potential benefit must
  audio-visual speaker                            be verified against
  detection                                       actual target footage.

  LightGBM / XGBoost      Defer until sufficient  A learned ranker needs
                          feedback                enough high-quality
                                                  labels and a reliable
                                                  evaluation set.
  -----------------------------------------------------------------------

## 6. Suggested Internal Interfaces

Use shared, versioned structures rather than passing loosely formatted
strings between services. Adapt names and fields to existing conventions
after inspecting the code.

### Candidate

``` ts
type ClipCandidate = {
  id: string;
  sourceId: string;
  startSeconds: number;
  endSeconds: number;
  transcriptText: string;
  hookText?: string;
  score: ClipQualityScore;
  scoreVersion: number;
  warnings: string[];
};
```

### Quality score

``` ts
type ClipQualityScore = {
  hook: number | null;
  coherence: number | null;
  contextIndependence: number | null;
  payoff: number | null;
  speechQuality: number | null;
  visualQuality: number | null;
  boundaryQuality: number | null;
  redundancyPenalty: number;
  riskPenalty: number;
  total: number;
  version: number;
};
```

### Tracking result

``` ts
type TrackingSample = {
  timestampSeconds: number;
  subjectId?: string;
  box?: { x: number; y: number; width: number; height: number };
  confidence: number;
  source: "vision" | "fallback";
};
```

These examples are conceptual contracts, not instructions to duplicate
existing types. First locate current structs and TypeScript interfaces,
then extend or consolidate them to avoid competing schemas.

## 7. Reliability, Security, and Error Handling

-   Keep API keys in the OS keyring or existing secure credential
    mechanism.
-   Never expose secret values in logs, errors, analytics, or generated
    reports.
-   Do not persist raw provider responses unless necessary; sanitize and
    bound diagnostic data.
-   Use timeouts and cancellation for network and media tasks.
-   Distinguish authentication errors, quota/rate-limit errors, network
    failures, malformed model output, and media failures.
-   Use bounded concurrency for expensive render tasks and make queue
    state explicit.
-   Ensure cancellation cleans up temporary files and does not leave
    projects stuck.
-   Use database migrations for schema changes and test upgrades from an
    existing user database.
-   Validate all model-generated timestamps, scores, IDs, and text
    before use.
-   Keep platform-specific code behind clear capability checks.
-   Avoid logging transcript or source-video content unless the user
    explicitly opts into diagnostic capture.

## 8. Testing and Benchmark Plan

### Unit tests

-   Timestamp and duration validation.
-   Transcript normalization.
-   Clip boundary padding and trimming.
-   Score normalization and weight handling.
-   Duplicate-candidate detection.
-   Crop geometry and safe margins.
-   Tracking smoothing and identity-switch behavior.
-   Subtitle offset and timing validation.
-   Provider response parsing and malformed-output handling.

### Integration tests

-   Import → transcribe → candidate generation.
-   Candidate selection → boundary refinement → render.
-   Center crop and face-tracking render paths.
-   Captioned render → FFprobe validation.
-   Render cancellation and retry.
-   Missing credentials and provider outage handling.
-   YouTube and Instagram preflight validation without publishing real
    content.

### Regression benchmark

Maintain a small, permission-cleared benchmark set with expected
annotations. For each change, record:

-   Human-rated hook quality.
-   Human-rated coherence and context independence.
-   Boundary correction frequency.
-   Word-cut or mid-sentence-cut frequency.
-   Active-speaker accuracy and identity switches.
-   Crop jitter and subject visibility.
-   Caption timing/layout failures.
-   Render success rate and processing time.

Compare against the baseline. Do not claim quality improvements based
only on a few visually impressive examples.

## 9. Implementation Order and Deliverables

  --------------------------------------------------------------------------------
  Phase                   Work                    Deliverable
  ----------------------- ----------------------- --------------------------------
  0                       Repository and pipeline `docs/AI_PIPELINE_BASELINE.md`
                          audit                   

  1                       Benchmark set and       `docs/AI_QUALITY_BENCHMARK.md`
                          baseline metrics        

  2                       Structured candidate    Candidate schema, validation,
                          output and hook scoring tests

  3                       Boundary refinement and Boundary service updates and
                          context checks          regression tests

  4                       Central quality scoring Shared scorer, versioned
                                                  weights, score breakdown

  5                       Tracking and crop       Tracking improvements behind
                          stability               controlled flags

  6                       Caption and             Validation checks and actionable
                          rendered-output         UI errors
                          validation              

  7                       Feedback capture and    Opt-in feedback schema and
                          evaluation              analysis workflow

  8                       Optional model          Benchmark reports with
                          experiments             keep/reject decision
  --------------------------------------------------------------------------------

Keep each phase in small pull requests. Feature flags are useful for
experimental tracking or ranking paths, but avoid creating flags for
every trivial change.

## 10. Definition of Done

The upgrade is complete only when:

-   The existing architecture is understood and documented.
-   Each change has a clear problem statement and baseline.
-   Candidate results are validated and scored consistently.
-   Hook quality, coherence, context independence, and boundary quality
    are separately measurable.
-   Reframing preserves subject visibility and avoids unnecessary jumps.
-   Caption and render outputs pass automated validation.
-   Provider failures and cancellation leave the project in a
    recoverable state.
-   Security-sensitive values remain protected.
-   Unit, integration, and regression tests pass.
-   Benchmark results demonstrate improvement without unacceptable
    regressions in speed or reliability.
-   Optional technologies are adopted only when their measured benefit
    exceeds their maintenance cost.

## 11. Instructions for the Implementing Engineer or Coding Agent

1.  Read the repository and trace the actual pipeline before modifying
    code.
2.  Inspect the existing implementations of candidate generation,
    transcription, boundary editing, tracking, crop calculation,
    subtitles, rendering, persistence, and publishing.
3.  Reuse existing abstractions and consolidate duplicates before
    creating new services.
4.  Do not replace Rust, Tauri, React, SQLite, FFmpeg, or Apple Vision
    as a first step.
5.  Implement the roadmap in priority order, starting with measurement
    and tests.
6.  For every phase, explain the problem, files changed, design
    decision, tests run, benchmark result, and remaining limitations.
7.  Run the relevant test suite and build checks after each meaningful
    change.
8.  Never claim a test or benchmark passed unless it was actually run.
9.  Do not use real YouTube or Instagram publishing as a test; use
    mocks, dry runs, or private test targets with explicit
    authorization.
10. Stop and document blockers when repository evidence contradicts an
    assumption instead of inventing APIs or silently rewriting the
    architecture.

------------------------------------------------------------------------

## Final Direction

**Make ClipOn smarter through measurable, incremental engineering---not
through a wholesale stack rewrite.** Prioritize better clip selection,
stronger openings, clean boundaries, context independence, stable
subject tracking, reliable captions, and validated renders. Keep the
current stack unless real benchmark evidence shows that a specific
component is limiting quality.
