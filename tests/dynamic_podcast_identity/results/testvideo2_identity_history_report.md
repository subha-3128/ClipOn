# Issue #3 Identity-History Verification Report

Date: 2026-10-06
Overall status: UNRESOLVED — RE-IDENTIFICATION IMPROVED, ACCEPTANCE STILL FAILS

## Change Tested

`src-tauri/bin/face_tracker.swift` now keeps up to eight appearance prototypes per identity, including pixel, optional landmark, Vision feature-print, and size/confidence data. Unmatched candidates are compared against confirmed temporary-lost and exited identities before new IDs are created. Strong historical matches revive exited tracks as `reappeared` immediately instead of waiting for three fresh hits.

The successful detector fallback and weighted association architecture were preserved. No audio or diarization signal was used.

## Full Runs

### testvideo2.mp4

- Duration: 679.463067 seconds
- Resolution: 640x360
- Frame rate: 30000/1001
- Raw JSON: `testvideo2_identity_history.json`
- Metadata: `testvideo2_identity_history.metadata.txt`
- IDs: `[1,2,3]`
- Segments: 86
- Layout counts: `single=42`, `split_two=7`, `split_three=37`

Track state totals:

- Person 1: 2,154 visible; 225 temporarily lost; 135 reappeared; 0 exited
- Person 2: 426 visible; 680 temporarily lost; 47 reappeared; 1,379 exited
- Person 3: 394 visible; 644 temporarily lost; 44 reappeared; 1,441 exited

Representative physical-ID continuity improved substantially:

- At 0.286s: ID 1 right, ID 2 center, ID 3 left.
- At 5.714s close-up: ID 1 remains the close-up subject; IDs 2 and 3 are temporarily lost.
- At 16s wide return: ID 1 right, ID 2 center, ID 3 left.
- At 42.857s, 93.714s, 202.857s, 350.286s, and 602.571s: IDs continue to map consistently to the same right/center/left physical participants.

Remaining failures:

- Two `split_two` segments use `[1,3]` instead of `[1,2]`, at approximately `138.3-142.0s` and `310.0-313.1s`.
- Person 2 and Person 3 still have long exited intervals in the full clip.
- These intervals require visual review before they can be accepted as correct exits rather than identity loss.

### testvideo.mp4 regression

- Duration: 676.033333 seconds
- Resolution: 1280x720
- Frame rate: 60
- Raw JSON: `testvideo_identity_history.json`
- Metadata: `testvideo_identity_history.metadata.txt`
- IDs: `[1,2,3]`
- Segments: 16
- Layout counts: `single=9`, `split_two=6`, `split_three=1`

Regression remains unresolved:

- Person 1: 543 visible keyframes
- Person 2: 39 visible keyframes
- Person 3: 1,412 visible keyframes
- `split_two` uses `[1,3]` at approximately `290.6-294.0s`, `556.0-569.1s`, and `602.6-615.1s`.
- This differs from the previous baseline and does not yet prove correct physical identity mapping.

## Acceptance Results

- Three-person persistent IDs: **FAIL**. Strongly improved, but two `[1,3]` intervals and long exited states remain.
- Close-up preserves existing identity: **PASS for representative early wide/close/wide sequence**.
- Wide return restores IDs: **PASS for representative checks through 602.571s**.
- Person 2 is not repeatedly replaced: **IMPROVED but FAIL**; Person 2 still becomes exited and is absent from two-person layouts.
- `split_two` contains correct IDs: **FAIL** for the two `[1,3]` testvideo2 intervals.
- `split_three` contains correct IDs: **PASS for 37 emitted segments in observed three-person sections**, pending the remaining physical identity failures.
- testvideo regression: **FAIL / NOT ACCEPTED**.
- Association scores: N/A; the tracker does not emit them.
- Duplicate numeric IDs: none.
- Issue #3: **UNRESOLVED — FAIL**.
- Issue #4: **MUST NOT BEGIN**.

No roadmap status was changed.
