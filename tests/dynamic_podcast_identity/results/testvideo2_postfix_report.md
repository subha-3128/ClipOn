# Issue #3 Post-Fix Verification Report

Date: 2026-10-06
Overall status: UNRESOLVED — DETECTION IMPROVED, IDENTITY ACCEPTANCE STILL FAILS

## Production Change Tested

`src-tauri/bin/face_tracker.swift` was updated to:

- Detect human and face rectangles instead of requiring landmarks for candidate creation.
- Use pixel, Vision feature-print, and optional landmark appearance evidence.
- Preserve the existing weighted association, motion, size, confidence, occlusion, timeout, and layout logic.
- Add scene-position history for multi-person re-association.

The production binary was rebuilt from this source before the final runs.

## Inputs and Artifacts

### testvideo2.mp4

- Format: H.264/AAC MP4
- Duration: 679.463067 seconds
- Resolution: 640x360
- Frame rate: 30000/1001
- Raw output: `testvideo2_postfix.json`
- Metadata: `testvideo2_postfix.metadata.txt`

### testvideo.mp4 regression

- Format: H.264/AAC MP4
- Duration: 676.033333 seconds
- Resolution: 1280x720
- Frame rate: 60/1
- Raw output: `testvideo_postfix.json`
- Metadata: `testvideo_postfix.metadata.txt`

Association scores remain N/A because the tracker does not emit them.

## testvideo2 Result

The tracker now emits IDs `[1, 2, 3]`, 75 layout segments, and all three layout types:

```text
single: 38
split_two: 34
split_three: 3
```

This confirms the detector fallback can activate additional people and the segmentation code can emit `split_three`.

However, identity persistence is not acceptable:

- Person 1: 2,125 visible keyframes
- Person 2: only 46 visible keyframes; 2,285 exited keyframes
- Person 3: 391 visible keyframes; 1,495 exited keyframes
- 34 `split_two` segments use `[1,3]` instead of the expected stable participant pair
- Person 2 disappears from most three-person sections
- Re-identification is not stable across cuts and shot changes

Representative position checks showed IDs can remain stable through an early wide/close/wide sequence after scene-anchor matching, but later sections still lose or replace Person 2. The improvement is therefore partial and does not satisfy persistent three-person identity acceptance.

## testvideo Regression Result

The regression run emits IDs `[1,2,3]`, 12 segments, and one split-three segment. This differs from the previous two-ID output and is not yet accepted as a regression-free result because the original report did not establish that the source never contains a third participant, and the new output has fragmented tracks:

- Person 1: 45 visible keyframes
- Person 2: 26 visible keyframes
- Person 3: 43 visible keyframes
- Most of the clip is `exited` for all three tracks

The existing one-/two-person acceptance evidence is therefore not revalidated by the current implementation.

## Scenario Results

- Three-person tracking: **FAIL**. Three IDs and split-three are emitted, but persistent identities are not maintained; many layouts use `[1,3]` and Person 2 is mostly exited.
- People changing positions/crossing: **BLOCKED — SCENARIO NOT PRESENT**. No controlled crossing was demonstrated.
- Temporary physical occlusion: **BLOCKED — SCENARIO NOT PRESENT**.
- Re-entry within 3.5 seconds: **BLOCKED — SCENARIO NOT PRESENT** as a controlled physical event.
- Re-entry after 3.5 seconds: **BLOCKED — SCENARIO NOT PRESENT** as a controlled physical event.
- Camera pan/zoom/reframing: **BLOCKED — SCENARIO NOT PRESENT** as a continuous motion event.
- Similar-looking people crossing: **BLOCKED — SCENARIO NOT PRESENT**.
- Existing one-/two-person regression: **FAIL / NOT ACCEPTED** because the current full run creates fragmented third-track output and differs from the previous baseline.

## Final Decision

- Detection improvement: demonstrated.
- Three-person layout generation: demonstrated.
- Persistent three-person identity: failed.
- Identity swaps/fragmentation: unresolved; `[1,3]` layout substitution and long Person 2 exits are evidence of incorrect persistence.
- Duplicate numeric IDs: none emitted.
- Two people sharing one numeric ID: not conclusively established.
- One person receiving multiple numeric IDs: identity fragmentation is present, but exact physical-ID mapping requires more controlled evidence.
- Issue #3: **UNRESOLVED — FAIL**
- Issue #4: **MUST NOT BEGIN**

The roadmap was not modified. No acceptance claim is made from the presence of three IDs alone.
