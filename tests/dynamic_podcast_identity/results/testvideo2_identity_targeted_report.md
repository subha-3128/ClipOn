# Issue #3 Targeted Lifecycle/Re-identification Report

Date: 2026-10-06
Status: UNRESOLVED — IDENTITY ACCEPTANCE STILL FAILS

## Targeted Changes

- Added bounded appearance prototype history per identity.
- Compared unmatched candidates against confirmed temporary-lost/exited identities before new-ID creation.
- Revived strong exited-track matches immediately as `reappeared`.
- Prevented exited tracks from using the ordinary association path.
- Added a context-sensitive initial-scene tie-breaker only when multiple candidates are visible.
- Prevented low-confidence candidates from reviving long-exited identities.
- Preserved detector fallback, weighted association, timeout, occlusion, and layout architecture.

## Final testvideo2 Run

- Input: `testvideo2.mp4`
- Output: `testvideo2_identity_targeted.json`
- IDs: `[1,2,3]`
- Segments: 76
- Layouts: `single=35`, `split_two=32`, `split_three=9`
- Association scores: N/A in JSON; temporary diagnostics were used during focused debugging and removed.

Remaining incorrect `split_two` participant IDs:

```text
55.7-58.9   [1,3]
310.0-315.1 [1,3]
394.3-398.9 [1,3]
596.6-599.7 [1,3]
```

Final lifecycle totals:

- Person 1: 2,142 visible; 236 temporarily lost; 136 reappeared
- Person 2: 322 visible; 632 temporarily lost; 1,456 exited; 86 reappeared
- Person 3: 145 visible; 208 temporarily lost; 2,033 exited; 29 reappeared

## Confirmed Root Causes and Fixes

The 310s trace demonstrated a correctly detected Person 2 candidate being matched through the ordinary path while the track remained `.exited`; `isActiveInScene` excluded it until later hits, allowing `[1,3]`. Exited matches are now routed through immediate re-identification/revival.

The 34.9s trace demonstrated a low-confidence false candidate reviving exited Person 3. Exited tracks are now re-identification-only, and low-confidence exited reactivation is rejected. That interval no longer appeared in the focused run.

The 138.3-142.0s trace showed an ambiguous candidate scoring higher for Person 3 than Person 2. Context-sensitive initial-scene history corrected that interval in focused testing.

## Remaining Failures

The full run still produces four `[1,3]` intervals. These require additional per-interval frame and score inspection before any further production change is justified. Person 2 remains exited for most of the full clip, and the identity acceptance criterion is not met.

The full `testvideo.mp4` regression also remains failed:

- IDs `[1,2,3]`
- 18 segments
- `split_two` intervals `[1,3]` at approximately `268.3-284.9s`, `556.0-569.1s`, and `602.0-615.1s`
- Person 2 has only 40 visible keyframes
- Person 3 has 1,380 visible keyframes

## Acceptance

- Stable Person 1/2/3: **FAIL**
- Close-up identity preservation: **PARTIAL**
- Wide-shot return: **PARTIAL**
- No replacement of Person 2: **FAIL**
- Correct split-two IDs: **FAIL**
- Correct split-three IDs: **PARTIAL**
- Existing testvideo regression: **FAIL**
- Issue #3: **UNRESOLVED**
- Issue #4: **MUST NOT BEGIN**

The roadmap was not modified.
