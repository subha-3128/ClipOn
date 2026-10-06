# Issue #3 Identity Verification Report

Date: 2026-10-06
Status: BLOCKED — REQUIRED SCENARIOS NOT PRESENT

## Test

- Input file: `testvideo.mp4`
- Format: MP4, H.264 video, AAC audio
- Duration: 676.033333 seconds
- Resolution: 1280x720
- Frame rate: 60/1 average and nominal
- Tracker window: 0-676.033 seconds
- Detected people: 2 unique IDs, `[1, 2]`
- Association scores: N/A; the tracker does not emit association scores
- Raw JSON: `testvideo.json`
- Media metadata: `testvideo.metadata.txt`
- Visual evidence: `testvideo_contact_sheet.jpg`, `testvideo_events_contact_sheet.jpg`
- Input SHA-256: `a6cad894f5f41549d0e4d355160975eaa64e4a4ee1bf00b8d2d65be6a817c008`
- Raw JSON SHA-256: `18d990a3367d6e4930b298b434201c03e35f125c798394daabb4fc82d3162e93`

## Tracker Evidence

The tracker emitted 2 people and 17 segments. Every segment was either `single` or `split_two`; maximum detected layout count was 2.

Person 1 had 2,365 keyframes and Person 2 had 2,213 keyframes. The only IDs in the output were 1 and 2, with no duplicate numeric IDs. The visual samples consistently show Person 1 as the woman and Person 2 as the man in split scenes.

Relevant layout sequence:

```text
single 0.000-44.000       ids [1]
split_two 44.000-80.857   ids [1, 2]
single 80.857-94.000     ids [1]
split_two 94.000-140.286 ids [1, 2]
single 140.286-170.286   ids [1]
split_two 170.286-198.571 ids [1, 2]
single 198.571-268.857  ids [1]
split_two 268.857-288.000 ids [1, 2]
single 288.000-358.286   ids [1]
split_two 358.286-377.714 ids [1, 2]
single 377.714-434.000   ids [1]
split_two 434.000-452.286 ids [1, 2]
single 452.286-556.571   ids [1]
split_two 556.571-569.143 ids [1, 2]
single 569.143-602.571   ids [1]
split_two 602.571-615.143 ids [1, 2]
single 615.143-676.033   ids [1]
```

## Scenario Results

### 1. One person

- Result: PASS for the one-person portions present in this video
- Expected: Person 1 remains stable with no duplicate IDs
- Actual: Person 1 remains ID 1 across the full output. No duplicate IDs were emitted.
- Evidence: `testvideo.json`, `segments[]`, Person 1 keyframes
- Problems: The clip is not a dedicated one-person acceptance clip; it contains edited single-person portions.

### 2. Two people

- Result: PASS for the two-person portions present in this video
- Expected: Person 1 and Person 2 remain consistent without swaps, merges, or splits
- Actual: Split segments consistently use IDs `[1, 2]`. Visual evidence identifies Person 1 as the woman and Person 2 as the man. No identity swap, duplicate ID, merge, or split was observed in the supplied footage.
- Evidence: `testvideo.json`, all `split_two` segments, `testvideo_contact_sheet.jpg`
- Problems: The footage uses editorial camera cuts and does not show the people crossing or exchanging positions.

### 3. Three people

- Result: BLOCKED — SCENARIO NOT PRESENT
- Expected: Stable IDs 1, 2, and 3
- Actual: Maximum detected people/layout count was 2. No three-person evidence exists.
- Evidence: `people[]` contains only IDs 1 and 2; no `split_three` segment.

### 4. Temporary occlusion

- Result: BLOCKED — SCENARIO NOT PRESENT
- Expected: A person is physically hidden behind another and returns with the same ID
- Actual: The video contains single-person editorial cuts, not a confirmed physical occlusion. At approximately `281.429-284.000s`, ID 2 is `temporarilyLost`; at `284.286s` it briefly reports `reappeared` with a tiny bounding box while the sampled frame does not visibly confirm the returning person, then becomes temporarily lost again. This is not sufficient evidence of successful occlusion handling.
- Evidence: Person 2 keyframes around `281.429-288.000s`; `testvideo_events_contact_sheet.jpg`
- Problems: The brief reappearance is visually unconfirmed and may be a false detection; it cannot be counted as a PASS.

### 5. Exit and re-entry within the configured lost-person timeout

- Result: BLOCKED — SCENARIO NOT PRESENT
- Expected: A real person exits and returns within `lostTimeoutSec = 3.5s` with the same ID
- Actual: The raw tracker has a 2.857-second temporary-loss interval for ID 2, but the footage does not clearly demonstrate a physical exit and return. The apparent reappearance has an unusually small box and is not visually confirmed.
- Evidence: Person 2 keyframes around `281.429-284.286s`

### 6. Exit and re-entry after the configured lost-person timeout

- Result: BLOCKED — SCENARIO NOT PRESENT
- Expected: Document the configured policy after a real exit longer than 3.5 seconds
- Actual: ID 2 has repeated `exited` states and later becomes visible again with ID 2 after long editorial single-person intervals, including returns near `94.000s`, `170.286s`, `358.286s`, `434.000s`, `556.571s`, and `602.571s`. These are edited shot changes, not controlled physical exit/re-entry events, so they cannot establish the intended lost-person policy.
- Evidence: Person 2 state transitions and split segments in `testvideo.json`

### 7. Camera movement / reframing

- Result: BLOCKED — SCENARIO NOT PRESENT
- Expected: Stable identities during a continuous pan, zoom, or reframing move
- Actual: The contact sheets show different static camera shots and editorial cuts, but no continuous pan, zoom, or reframing sequence suitable for acceptance.
- Evidence: `testvideo_contact_sheet.jpg`

### 8. Similar-looking people crossing

- Result: BLOCKED — SCENARIO NOT PRESENT
- Expected: Similar-looking people cross/change positions without an ID swap
- Actual: The clip contains one woman and one man. No similar-looking pair crossing or position exchange is present.
- Evidence: `testvideo_contact_sheet.jpg`

## Summary

- Tests executed: 1 full 676.033-second tracker run
- Tests passed: 1-person portions; 2-person portions without crossing
- Tests failed: none conclusively demonstrated
- Tests blocked: three people, physical occlusion, controlled re-entry within timeout, controlled re-entry after timeout, camera movement, similar-looking people crossing
- Identity swaps found: none in the observed output
- Duplicate IDs found: none; IDs were `[1, 2]`
- Incorrect new IDs: none observed; controlled re-entry was not present
- Merging/splitting: none observed in applicable two-person sections
- Occlusion behavior: not accepted; the only short event is visually unconfirmed
- Re-entry behavior: same ID 2 reappears after edited single-person intervals, but controlled policy behavior is unverified
- Camera-motion behavior: untested; only editorial shot changes are present
- Similar-face behavior: untested
- Issue #3 status: **BLOCKED**
- Issue #4 can begin: **No**

No production code or roadmap status was changed after this test run.
