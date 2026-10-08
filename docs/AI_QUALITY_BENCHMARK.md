# ClipOn AI Quality Benchmark & Ground Truth Evaluation (Phase 1)

**Date:** 2026-10-09  
**Version:** 1.0.0  
**Scope:** Test dataset definitions, evaluation criteria, ground-truth annotations, and scoring metrics for automated and regression evaluation.

---

## 1. Benchmark Dataset Catalog

The benchmark comprises representative video profiles across the most common YouTube Shorts & Instagram Reels genres:

| ID | Category | Characteristics | Duration | Target Challenges |
|---|---|---|---|---|
| `BM-POD-01` | **Two-Person Podcast** | Two seated speakers, wide shot with switching dialogue | 180s | Active speaker switching, multi-person crop, conversational continuity. |
| `BM-INT-02` | **Studio Interview** | Guest and host, alternating angles, lapel audio | 240s | Pronoun context, payoff delivery, question-and-answer boundary. |
| `BM-SOLO-03` | **Solo Educational / Tech** | Centered presenter, dynamic hand gestures, head movement | 120s | Crop jitter reduction, hook energy in first 2s, kinetic caption sync. |
| `BM-SCRN-04` | **Screen Recording + Facecam** | Small webcam corner overlay with desktop slides/code | 90s | Visual ROI detection, maintaining presenter visibility vs screen detail. |
| `BM-LOWQ-05` | **Low-Quality / Mobile Video** | Variable lighting, acoustic reverb, hand-held camera shake | 60s | Noisy audio RMS, face tracker re-acquisition under motion blur. |
| `BM-FAST-06` | **Rapid Comedy / Narrative** | Fast-paced speech, overlapping dialogue, punchline timing | 90s | Sentence cutoff prevention, punchline retention, zero dead air. |

---

## 2. Evaluation Criteria & Scoring Matrix

Every candidate clip is evaluated against 8 core dimensions on a normalized 0.0 to 1.0 scale:

### 1. Hook Strength (`H`)
- **Measurement:** First 1.5–2.0 seconds of speech and audio.
- **Positive Indicators:** Contrarian statement ("Stop doing X"), direct question ("Why does everyone..."), quantifiable curiosity ("Here are 3 reasons..."), high RMS energy onset.
- **Negative Indicators:** Filler lead-in ("So basically...", "Um, yeah"), intro pleasantries ("Hey guys welcome back"), dead air > 0.4s.

### 2. Coherence & Context Independence (`C`)
- **Measurement:** Standalone comprehensibility without seeing the full video.
- **Positive Indicators:** Clear topic introduction, explicit subject nouns before pronouns, single self-contained idea or thesis.
- **Negative Indicators:** Unresolved demonstratives ("And that's why he did that"), dangling premises without conclusions.

### 3. Payoff & Conclusion (`P`)
- **Measurement:** Ending 3–5 seconds of the clip.
- **Positive Indicators:** Natural resolution, comedic punchline, actionable takeaway, crisp statement followed by natural pause.
- **Negative Indicators:** Mid-sentence cut-off, speaker inhaling for the next sentence, trailing thoughts.

### 4. Boundary Accuracy (`B`)
- **Measurement:** Word-level alignment at start and end.
- **Standard:** Zero clipped phonemes (starts ≥ 80ms before first word, ends ≥ 150ms after final word; never cuts inside a word).

### 5. Subject & Active-Speaker Tracking (`T`)
- **Measurement:** Visual subject continuity in vertical 9:16 framing.
- **Standard:** Correct speaker framed ≥ 90% of speaking time; identity preserved during head turns; smooth pan without sudden teleports.

### 6. Crop Stability & Composition (`S`)
- **Measurement:** Camera motion stability.
- **Standard:** Head and shoulders centered within top 60% vertical safe zone; zero jitter during stationary speech; smooth transition on camera cuts.

### 7. Caption Synchronization & Readability (`K`)
- **Measurement:** Kinetic word highlights vs audio waveform.
- **Standard:** Word highlight error ≤ 50ms; lines fit within horizontal 80% safe zone; no text obscured by UI overlays (reels caption area).

### 8. Render & Video Quality (`R`)
- **Measurement:** Codec, resolution, framerate, and audio fidelity.
- **Standard:** Constant 1080x1920 (9:16), clean hardware encode, audio normalized to -14 LUFS (EBU R128).

---

## 3. Ground Truth Test Suites

The benchmark test suite is automated via Rust integration and unit tests:

1. **Synthetic Transcript Unit Tests**:
   - `test_detect_candidates_with_provider_mock`: Verifies structured parsing.
   - `test_snap_candidates_enforces_60s_maximum_and_targets_sweet_spot`: Verifies boundary duration rules.
   - `test_clean_leading_discourse_marker`: Verifies filler elimination without semantic destruction.
2. **Audio Waveform & Energy Benchmarks**:
   - `test_audio_extraction_meta_serde_and_cache_validation`: Validates 16kHz audio extraction.
   - `test_scoring_components_normalization_bounds`: Validates mathematical bounds of all scoring components.
3. **Visual & Active Speaker Evidence Benchmarks**:
   - `test_bipartite_mapping_prevents_multiple_speakers_collapsing`: Tests multi-speaker tracking.
   - `test_temporal_smoothing_bridges_speech_gaps_and_prunes_spikes`: Tests crop smoothing under momentary occlusions.

---

## 4. Benchmark Execution & Regression Protocol

When running quality evaluations across candidate changes:
1. Run `cargo test --lib` for deterministic regression checks.
2. Generate synthetic and sampled clips on the catalog profiles.
3. Verify metrics against baseline targets.
4. If a proposed algorithm change reduces active-speaker accuracy or introduces crop jitter, it must be rejected or placed behind a configurable flag.
