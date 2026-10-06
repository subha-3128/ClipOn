# Dynamic Podcast Identity Verification

Status: BLOCKED - TEST FOOTAGE REQUIRED

This directory is the execution plan and runner for Issue #3. It intentionally contains no synthetic acceptance results. Do not mark a scenario PASS unless the tracker has been run against the required real footage and the raw JSON has been reviewed.

## Prerequisites

- macOS with Vision, AppKit, Accelerate, Swift, FFmpeg, and FFprobe
- A real video supplied for the scenario under test
- The production tracker binary at `src-tauri/bin/clipon-face-tracker`, or a freshly rebuilt binary from `src-tauri/bin/face_tracker.swift`
- No API credentials are needed for tracker verification

## Build the Tracker

From the repository root:

```sh
swiftc src-tauri/bin/face_tracker.swift \
  -o /tmp/clipon-face-tracker-identity-test \
  -framework Vision \
  -framework AppKit \
  -framework Accelerate
```

Use the temporary binary for a test run so the tested source and executable are unambiguous.

## Run One Analysis

```sh
./tests/dynamic_podcast_identity/run_tracker.sh \
  /tmp/clipon-face-tracker-identity-test \
  /path/to/input.mp4 \
  0 \
  60 \
  tests/dynamic_podcast_identity/results/one_person.json
```

The runner writes:

- Raw tracker JSON at the requested output path
- `*.metadata.txt` containing input path, start, duration, media duration, dimensions, frame rate, and stream information
- A structural summary printed to stdout

The runner does not decide identity correctness. Review the raw `people[].keyframes` and `segments[]` data against the scenario acceptance criteria.

## Required Scenarios

| ID     | Scenario                         | Minimum evidence                                                                                                             |
| ------ | -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| ID-01  | One person                       | 30-60 seconds; one visible person; stable framing; no duplicate IDs                                                          |
| ID-02  | Two people                       | 60-90 seconds; both people move, cross, and change relative position; IDs remain stable                                      |
| ID-03  | Three people                     | 90-120 seconds; all three people move and change relative position; IDs remain stable                                        |
| ID-04  | Temporary occlusion              | Two people; one person is hidden behind the other for less than the configured 3.5 second timeout, then returns              |
| ID-05A | Exit and re-entry within timeout | Person leaves the frame and returns before 3.5 seconds; identity must be reassociated                                        |
| ID-05B | Exit and re-entry after timeout  | Person leaves longer than 3.5 seconds and returns; record whether a new identity is expected by the configured state machine |
| ID-06  | Camera movement                  | One or more people during pan, zoom, or reframing; no identity swap                                                          |
| ID-07  | Similar-looking people           | Two or more visually similar people; crossing and re-entry; no identity swap or duplicate                                    |

The 3.5 second value comes from `lostTimeoutSec` in the tracker. Record the actual clip timing and do not silently change this threshold during testing.

## Per-Scenario Capture Form

Copy this form once for every scenario and fill it from observed output:

```text
Scenario:
Result: PASS / FAIL / BLOCKED
Input video:
Input video hash:
Duration analyzed:
Start offset:
Number of detected people:
Person IDs over time:
Identity changes:
Identity swaps:
Duplicate IDs:
Merges or splits:
Occlusion events:
Re-entry events:
Configured lost-person timeout:
Observed re-association behavior:
Association confidence/scores: N/A (not emitted by current tracker) / attached evidence
Layout changes:
Raw JSON:
Metadata:
Notes:
```

## Acceptance Rules

- A missing or ambiguous identity is not a PASS.
- A new ID after a temporary occlusion is a FAIL.
- A person changing IDs after crossing, camera movement, or similar-person interaction is a FAIL.
- A duplicate track for one visible person is a FAIL.
- A layout change does not prove identity correctness; inspect IDs and keyframes separately.
- Speaker diarization must not be used as visual identity evidence.
- Keep the raw JSON and metadata beside every completed report.

## Required Evidence Before Issue #3 Can Close

Run all seven scenarios, including both exit/re-entry timings, and attach one completed capture form plus raw output for each. Report every observed ID transition, occlusion/re-entry event, layout segment, and failure. Only then may the roadmap checkbox for Issue #3 be changed.

Current result: BLOCKED - TEST FOOTAGE REQUIRED.
