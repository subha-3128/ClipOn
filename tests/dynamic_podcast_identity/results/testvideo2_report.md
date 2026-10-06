# Issue #3 Identity Verification Report: testvideo2

Date: 2026-10-06
Overall status: BLOCKED — THREE-PERSON ACCEPTANCE FAILED; OTHER REQUIRED SCENARIOS NOT PRESENT

## Test

- Input file: `testvideo2.mp4`
- Format: MP4, H.264 video, AAC audio
- Duration: 679.463067 seconds
- Resolution: 640x360
- Frame rate: 30000/1001 (29.97 FPS)
- Tracker window: 0-679.463 seconds
- Raw JSON: `testvideo2.json`
- Media metadata: `testvideo2.metadata.txt`
- Visual evidence: `testvideo2_contact_sheet.jpg`
- Association scores: N/A; the tracker does not emit association scores
- Input SHA-256: `e2e44004b9db3d5f4c30238e0dd56e52ed6160f3cd3620ab2091135c5f0256c8`
- Raw JSON SHA-256: `14152dde32e46fc6987467f2002caff423c572c2ba12369d530de6b17e8aa864`
- Contact sheet SHA-256: `88b11c1d61c1e055f4bc3b631b3c5654faadd409150e5e98b821ced7f0e782ef`

## Actual Tracker Output

The tracker emitted exactly two IDs: `[1, 2]`.

It emitted exactly one layout segment for the full clip:

```text
0.000-679.463 single, number_of_people=1, person_ids=[1]
```

Person 1 had 2,376 keyframes. Person 2 had 2,373 keyframes but was visible only briefly around 118.857-119.143 seconds and remained `exited` for the rest of the output. No `split_two` or `split_three` layout segment was emitted.

The visual contact sheet clearly contains wide shots with three people seated together, plus close shots of individual people. The tracker output does not represent those three-person wide shots.

A temporary Vision probe on a representative three-person wide frame found:

```text
VNDetectFaceRectanglesRequest: 2 observations
VNDetectFaceLandmarksRequest: 0 observations
```

This identifies the immediate failure surface as face detection at the supplied 640x360 wide-shot scale and pose, before identity association can operate. Upscaling a 0-30 second excerpt to 1280x720 still produced only one tracker ID, so simple source upscaling did not resolve the failure.

## Layout-Segment Discrepancy Investigation

This is not evidence that the footage contains no visual person-count changes. The contact sheet contains wide shots with three visible people. The one-segment result is explained by the tracker state consumed by segmentation:

- Person 1: 2,376 keyframes; 1,543 visible keyframes; state counts were `visible=1,348`, `temporarilyLost=735`, `reappeared=169`, `exited=115`, and `tentative=9`.
- Person 2: 2,373 keyframes but only 2 visible keyframes, at approximately `118.857s` and `119.143s`; state counts were `tentative=5` and `exited=2,368`.
- Person 2 never reached the `visible` state. Its two detections were emitted while the state was still `exited`.
- `isActiveInScene` returns true only for `visible`, `temporarilyLost`, or `reappeared`; an `exited` track is excluded from `activePersonIds`.
- Consequently, every `frameRecord.activePersonIds` consumed by segmentation contains only `[1]`.
- `validIds` therefore remains `[1]`, `targetIds` remains `[1]`, and `targetLayout` remains `single` for the complete timeline.
- No `1 -> 2`, `2 -> 3`, `3 -> 2`, or `2 -> 1` transition was generated. The 3-second minimum-duration hysteresis did not suppress a real candidate transition; no multi-person candidate reached the hysteresis check.
- The code can return `split_three` from `determineLayoutType` when three active IDs exist, but the current frame records never contain three active IDs. The JSON does not expose internal frame records or candidate layouts, so this conclusion is derived directly from the state counts and the implementation.

Therefore the single segment is expected from the tracker state it produced, but it is incorrect relative to the visible three-person footage. The failure occurs before layout segmentation, at face detection/identity activation, with a secondary lifecycle consequence that the detected Person 2 never becomes active.

## Scenario Results

### 1. Three-person tracking and persistent IDs

- Result: FAIL
- Expected: IDs 1, 2, and 3 remain stable in the wide three-person shots.
- Actual: The footage visibly contains three people, but the tracker emitted only IDs 1 and 2 and only a `single` layout. No ID 3 was produced.
- Failure: Three-person detection/layout is not functioning for the supplied wide shots.
- Segmentation diagnosis: The output is not a hysteresis failure. No candidate layout containing Person 2 or Person 3 reached segmentation because the active-ID list was always `[1]`.

### 2. People changing positions/crossing

- Result: BLOCKED — SCENARIO NOT PRESENT
- Expected: People physically cross or exchange positions.
- Actual: The footage shows seated people and editorial changes between wide and close shots; no continuous crossing or position exchange was present.

### 3. Temporary physical occlusion

- Result: BLOCKED — SCENARIO NOT PRESENT
- Expected: One person physically disappears behind another and returns.
- Actual: No controlled physical occlusion was present. The tracker has frequent temporary-loss states, but those are not visual proof of occlusion.

### 4. Re-entry within the configured lost-person timeout

- Result: BLOCKED — SCENARIO NOT PRESENT
- Expected: A person exits and returns within `lostTimeoutSec = 3.5s`.
- Actual: The footage does not contain a controlled, visually verifiable exit/re-entry event within the timeout.

### 5. Re-entry after the configured lost-person timeout

- Result: BLOCKED — SCENARIO NOT PRESENT
- Expected: A person exits for longer than 3.5 seconds and returns, allowing policy behavior to be evaluated.
- Actual: Editorial cuts and close-shot changes are present, but no controlled physical exit/re-entry event is available.

### 6. Camera pan/zoom/reframing

- Result: BLOCKED — SCENARIO NOT PRESENT
- Expected: Stable identities through continuous camera motion.
- Actual: The footage uses static shots and editorial cuts. No continuous pan, zoom, or reframing sequence was identified.

### 7. Similar-looking people crossing

- Result: BLOCKED — SCENARIO NOT PRESENT
- Expected: Similar-looking people cross or change positions without identity swaps.
- Actual: No similar-looking crossing scenario was present.

### 8. Identity persistence in available close-shot material

- Result: FAIL / INCONCLUSIVE
- Expected: A physical person retains one ID across the clip.
- Actual: Person 1 is repeatedly marked visible/temporarilyLost/reappeared and appears across different close-shot subjects, while Person 2 is detected only briefly. Because the tracker does not produce reliable concurrent identities for the visible three-person shots, no end-to-end identity correspondence can be accepted.
- Failure: The output is insufficient to prove identity persistence across the supplied footage.

## Combined Evidence With testvideo.mp4

`testvideo.mp4` passed the demonstrated one-person portions and two-person split portions, with IDs `[1, 2]`, no duplicate IDs, and no observed swaps. Its physical occlusion, controlled re-entry, camera-motion, similar-face, crossing, and three-person scenarios were blocked.

testvideo2.mp4 adds real three-person wide shots, but the tracker fails to detect and represent them. Therefore the combined evidence does not satisfy Issue #3.

## Summary

- Tests executed: 2 full tracker runs, plus a temporary Vision detector probe and an upscaled 0-30 second diagnostic run
- Tests passed: One-person and two-person portions demonstrated by `testvideo.mp4`
- Tests failed: Three-person tracking/layout on `testvideo2.mp4`; end-to-end identity persistence cannot be accepted for the three-person footage
- Tests blocked: Crossing/position exchange, physical occlusion, re-entry within timeout, re-entry after timeout, camera movement, similar-looking people
- Identity swaps found: No conclusively proven swap; identity correctness is not established for the failed three-person detection case
- Duplicate IDs found: None; emitted IDs were unique, but missing ID 3 is a failure
- ID fragmentation: Tracker state is fragmented by repeated loss/reappearance; controlled identity correctness is not proven
- Two people sharing one ID: Not conclusively proven from output
- One person receiving multiple IDs: Not conclusively proven from output
- Incorrect exits/re-entry: Not acceptance-testable because the footage lacks controlled physical events
- Layout noise: Tracker emitted `single` for visibly three-person wide shots; this is an incorrect layout result, not transient layout noise
- Layout transition evidence: No `1->2`, `2->3`, `3->2`, or `2->1` transitions were emitted.
- Issue #3 status: **UNRESOLVED — FAIL plus BLOCKED scenarios**
- Issue #4 can begin: **No**

No production code or roadmap status was changed during this test.
