# Issue #4 — Explicit Layout Confirmation Report

Date: 2026-10-06
Status: **PASSED** (all roadmap acceptance criteria verified)

## Scope

Implemented Issue #4 only. No changes to identity tracking (Issue #3 work preserved:
detector fallback, persistent IDs, prototype history, re-identification, exited-track
handling, scene-history), detector, renderer, or any unrelated module.

## Gap found (audit)

The layout state machine (`face_tracker.swift`, Step 3) tracked only
`currentLayout / currentPersonIds / currentStart` and committed a layout change on the
**first differing observation** once `minLayoutDurationSec` (3.0s) had elapsed in the
current layout. There was no candidate confirmation anywhere (Swift or Rust):

- A single 3.5 fps mis-observation (spurious detection or one-frame identity flap)
  flipped the layout immediately after the 3s gate, then pinned the wrong layout for
  ≥ 3s. Roadmap failure mode: `2,2,3,2,3,2 → switches to 3` instead of staying at 2.

## Implementation (`src-tauri/bin/face_tracker.swift`)

- New `layoutConfirmationSec = 1.0` engine config (~1s per roadmap).
- New `LayoutConfirmationMachine` struct tracking exactly the roadmap fields:
  `currentLayout`, `candidateLayout` (+ ids), `candidateStartTime`, `stableDuration`.
  A candidate differing from the committed (layout, person-id) pair must be observed
  **continuously for ≥ 1.0s** AND the current layout must have held ≥ 3.0s before the
  switch commits. Any observation matching the committed layout, or diverging from the
  candidate, resets the candidate window.
- Commit is back-dated to the candidate's first observation (every observation since
  then agreed with it), clamped to `currentStart + minLayoutDurationSec` so the previous
  segment never shrinks below 3.0s (protects Step 4's merge semantics and the renderer's
  one-ffmpeg-pass-per-segment contract).
- Step 3 rewritten to drive the machine; JSON output schema unchanged
  (`people[].keyframes`, `segments[]` contiguous, covering `[0, durationSec]`).
- Added `--selftest-layout` CLI mode (does not affect the normal 3-argument invocation)
  asserting the roadmap acceptance sequences.

## Self-test (`--selftest-layout`) — 7/7 PASS

| Case | Result |
| --- | --- |
| Roadmap `2,2,3,2,3,2 → stay at 2` (flap suppression) | PASS |
| Roadmap `3,3,3,3,3 → switch to 3` (stable switch) | PASS |
| Single differing observation after 3s ignored (old code committed here) | PASS |
| Person-id change within `split_two` requires the same confirmation | PASS |
| Interrupted candidate restarts its confirmation window | PASS |
| Differing observation with elapsed ≥ 3s but stable < 1s does not commit | PASS |
| Candidate observed before 3s: commit clamped so first segment stays ≥ 3.0s | PASS |

## Full-video regression (production binary rebuilt at `src-tauri/bin/clipon-face-tracker`)

### testvideo2.mp4 (0–679.463s)

- Segments 76 → **64**; layouts `single 35→30, split_two 32→26, split_three 9→8`.
- Removed segments are all ~3.1s blip commits, e.g. `15.4–18.6 [1,2]`, `110.3–113.4 single`,
  `199.7–202.9 single`, `663.4–666.6 single` — absorbed into their surrounding stable layouts.
  The previously reported incorrect `[1,3]` blips at 55.7–58.9s and 596.6–599.7s no longer
  commit; those windows now hold the sustained `split_three [1,2,3]` state.
- 486–502s improved: three 3.1s `split_three`/`single` flip-flops merged into coherent
  sustained windows (`485.9–488.9`, `491.9–500.3`).
- Remaining `split_two [1,3]` at 310.0–315.1s and 394.3–398.9s are sustained 4.6–5.1s
  states that pass confirmation (identity-layer evidence; Issue #3 scope, closed by
  owner decision — not revisited here).
- Person keyframe state totals **byte-identical** to the pre-change run
  (P1: 2005 visible / P2: 235 visible, 632 temporarilyLost, 1456 exited, 86 reappeared /
  P3: 115 visible, 208 temporarilyLost, 2033 exited, 29 reappeared) → identity pipeline
  untouched.
- Contiguity OK, full coverage `[0, 679.463]`, minimum segment duration exactly 3.00s.

### testvideo.mp4 (0–676.033s)

- Segments 18 → **17** (one blip commit gone); layouts `single 10→9, split_two 7, split_three 1`.
- Minimum segment duration 3.43s; contiguity OK; full coverage.
- The known `[1,3]` windows (268.3–284.9, 556.0–569.1, 602.0–615.1) are sustained states
  unchanged in character — pre-existing Issue #3-accepted evidence, not confirmation artifacts.

## Rust regression

- `cargo test`: **14 passed, 0 failed** (includes `media::tests::test_build_segment_filter_graph_split_two`
  and `_split_three` — the Podcast renderer integration tests).
- Renderer/JSON contract unchanged: segments contiguous, non-overlapping, cover the full
  analyzed window; `layout_type`/`person_ids`/`start`/`end` semantics identical.

## Audits

- `git diff --check`: clean.
- Diff scanned for `apiKey / secret / token / password / Bearer / /Users/`: no hits.
- Only `face_tracker.swift` (+ rebuilt tracker binary) changed by this issue; pre-existing
  working-tree changes from the Issue #3 effort were not touched.

## Verification boundary

End-to-end ffmpeg render of a segment timeline was not executed (no standalone render CLI;
rendering is exercised through the Tauri app). The renderer-facing contract is verified by
the 14 passing unit tests plus programmatic contiguity/coverage checks on both full runs.

## Acceptance

- New layout stays stable for several consecutive observations before switching: **PASS**
- `2,2,3,2,3,2 → stay at 2`: **PASS** (selftest + full runs)
- `3,3,3,3,3 → switch to 3`: **PASS** (selftest + full runs)
- ~1s confirmation window, tuned against real footage: **PASS** (1.0s; no over-suppression —
  all sustained layouts preserved)
- Existing hysteresis/min-duration preserved (all segments ≥ 3.0s): **PASS**
- No regression to identity tracking, layouts, or Podcast renderer integration: **PASS**
- Issue #4: **PASSED**
