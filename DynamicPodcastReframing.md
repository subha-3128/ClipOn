Continue working directly on the existing ClipOn codebase and implement the complete dynamic Podcast reframing system.

Do not create a separate application, do not replace the existing Shorts/Reels pipeline, and do not only explain the solution. Inspect the existing implementation and make the required code changes yourself.

The current Podcast Split Screen implementation is designed mainly around two people and static/average crop positions. Replace that approach with a proper multi-person, timeline-based tracking and dynamic layout system.

==================================================
GOAL
==================================================

The input is normally a 16:9 podcast video.

The output must be a 9:16 vertical video suitable for Instagram Reels and YouTube Shorts.

The layout must dynamically change throughout the video based on the number of people actually present in the video at that point in time.

Supported layouts:

1 person → full 9:16
2 people → two horizontal sections
3 people → two people on top + one person on bottom

Do NOT decide the layout from the first frame only.

The system must continuously analyze frames throughout the entire selected video/clip.

==================================================

1. CONTINUOUS PEOPLE DETECTION
   \==================================================

Analyze the video throughout the entire timeline.

Do not inspect only the first frame and assume the same number of people exists for the whole video.

People may:

- Enter the frame
- Leave the frame
- Temporarily disappear
- Become occluded
- Move around
- Return to the frame

The system must detect these changes over time.

Use multiple consecutive frames to confidently determine whether a person has actually entered or left.

Do not change the layout because of one missed face detection or one false detection.

For example:

00:00–00:15 → Person 1
00:15–00:35 → Person 1 + Person 2
00:35–00:55 → Person 1 + Person 2 + Person 3
00:55–01:20 → Person 1 + Person 2
01:20–01:40 → Person 1

The output layout must follow these changes automatically.

================================================== 2. PERSISTENT PERSON IDENTITY
==================================================

Do not treat every face detection as a new person.

Create persistent identities:

Person 1
Person 2
Person 3
...

Track each person across the timeline.

The same person must keep the same identity even when:

- They move
- Turn their head
- Change expression
- Stop speaking
- Temporarily become occluded
- Move closer/further from the camera

Do not use only left/right position or speaking status to determine identity.

Identity should primarily be based on temporal tracking across frames, supported by face appearance/landmarks, position, size, and confidence.

Never allow:

- Person 1 to become Person 2
- Person 2 to become Person 3
- The same person to appear in multiple sections
- A speaking person to replace a silent person
- A temporary detection to create an incorrect new identity

================================================== 3. TRACKING DATA MODEL
==================================================

The current implementation uses data such as:

person_1_keyframes
person_2_keyframes

This is too restrictive.

Change the tracking architecture to support a dynamic collection of people.

Conceptually, the tracking result should support:

PersonTrack[]
PersonTrack 1
PersonTrack 2
PersonTrack 3
...

Each person should have time-based tracking information containing at least:

- person ID
- timestamp
- face/crop position
- face/crop dimensions
- confidence
- visibility state

Do not hardcode the renderer to only Person 1 and Person 2.

================================================== 4. DYNAMIC LAYOUT SEGMENTS
==================================================

Create a timeline of layout segments.

Conceptually:

LayoutSegment {
start
end
number_of_people
person assignments
layout type
}

For example:

00:00–00:15
Single(Person 1)

00:15–00:35
Two(Person 1, Person 2)

00:35–00:55
Three(Person 1, Person 2, Person 3)

00:55–01:20
Two(Person 1, Person 2)

01:20–01:40
Single(Person 1)

The renderer must consume this timeline instead of assuming one layout for the entire video.

================================================== 5. ONE PERSON LAYOUT
==================================================

When only one person is present:

Use the entire 9:16 output.

Example:

┌─────────────────┐
│ │
│ │
│ PERSON 1 │
│ │
│ │
│ │
└─────────────────┘

Track the person's face and dynamically position the crop so the person remains properly framed.

================================================== 6. TWO PERSON LAYOUT
==================================================

When two people are present:

Split the 9:16 frame horizontally into two equal sections.

┌─────────────────┐
│ │
│ PERSON 1 │
│ │
├─────────────────┤
│ │
│ PERSON 2 │
│ │
└─────────────────┘

Top section → Person 1
Bottom section → Person 2

Each section is approximately 9:8.

Both sections must represent the exact same timestamp from the original 16:9 video.

Never show the same person in both sections.

================================================== 7. THREE PERSON LAYOUT
==================================================

When three people are present, DO NOT create three horizontal rows.

Use this layout:

┌─────────┬─────────┐
│ │ │
│ PERSON1 │ PERSON2 │
│ │ │
├─────────┴─────────┤
│ │
│ PERSON 3 │
│ │
└───────────────────┘

Top half:

- Person 1 → left
- Person 2 → right

Bottom half:

- Person 3 → full width

The two top sections should have equal width.

The bottom person should occupy the full width.

This layout must match the provided reference screenshots.

================================================== 8. TEMPORARY DISAPPEARANCE
==================================================

A temporary disappearance must NOT immediately trigger a layout change.

Example:

Person 1 temporarily moves outside the camera frame.

Do not immediately:

- Replace Person 1 with Person 2
- Duplicate Person 2
- Create a new person
- Swap identities
- Change the layout unnecessarily

Keep Person 1's identity and assigned section stable.

Maintain the last valid tracking/crop position or appropriate original-video framing until Person 1 becomes visible again.

When Person 1 returns, detect the same person and smoothly resume tracking.

Apply the same behavior to every tracked person.

Only change the layout when there is sufficient evidence that the actual composition of people has changed.

================================================== 9. FACE TRACKING
==================================================

The current static/average crop positions are not sufficient.

Replace them with dynamic per-segment tracking.

The tracking system must follow each person's face throughout the video.

Do not use one static crop position for the entire clip.

Smoothly interpolate between tracking positions to avoid:

- Jumping
- Shaking
- Sudden crop changes
- Incorrect framing

The face should remain properly framed within the person's assigned output section.

================================================== 10. TRACKING FREQUENCY
==================================================

The existing tracking approach is too sparse for reliable dynamic layout changes.

Increase the analysis frequency enough to reliably detect people entering/leaving and follow movement.

Use an efficient analysis rate rather than unnecessarily processing every full-resolution frame.

The important requirement is that the tracking timeline is dense enough to capture meaningful changes and that rendering remains smooth through interpolation.

================================================== 11. SPEECH RECOGNITION / DIARIZATION
==================================================

Use the existing transcription/speaker diarization system where appropriate.

Speech recognition should help determine:

- Who is speaking
- Speaker changes
- Caption timing
- Conversation timing

However:

SPEECH ACTIVITY MUST NOT BE THE PRIMARY METHOD OF PERSON IDENTITY.

Face/person tracking determines which visual person belongs to which section.

A silent person must remain visible in their assigned section.

Use the existing transcription infrastructure instead of creating an unnecessary duplicate transcription system.

================================================== 12. SAME TIMESTAMP
==================================================

Every section must represent the exact same timestamp from the original video.

For example:

Original 16:9 frame at 00:35.200

Person 1 crop → 00:35.200
Person 2 crop → 00:35.200
Person 3 crop → 00:35.200

Then combine them into ONE 9:16 frame.

Never allow the sections to use different timestamps.

Audio and video must remain synchronized.

================================================== 13. CAPTIONS
==================================================

Keep captions synchronized with the original speech timestamps.

Captions must continue working when the layout changes from:

1 person
→ 2 people
→ 3 people
→ 2 people
→ 1 person

For two people, captions can appear around the horizontal dividing line.

For three people, position captions so they remain readable without unnecessarily covering faces.

Do not break caption timing when the layout changes.

================================================== 14. MEDIA.RS / FFMPEG
==================================================

Update the existing Rust media/rendering pipeline.

The renderer must no longer assume:

person_1_keyframes
person_2_keyframes

only.

It must consume:

- Person tracks
- Layout segments
- Per-person crop positions
- Visibility information

Build the FFmpeg processing dynamically based on the layout timeline.

Do not create one huge unmaintainable FFmpeg expression if a segment-based rendering approach is cleaner.

A suitable architecture is:

Original video
↓
Tracking timeline
↓
Layout segments
↓
Render each segment using the correct layout
↓
Concatenate segments
↓
Apply remaining processing
↓
Captions
↓
Final 9:16 video

Make the implementation efficient and compatible with the existing Apple Silicon/VideoToolbox rendering pipeline.

================================================== 15. EXISTING FEATURES MUST CONTINUE WORKING
==================================================

Do not break existing ClipOn functionality.

Keep working:

- YouTube importing
- Transcription
- Viral moment detection
- Shorts/Reels generation
- Center Crop
- Original 16:9
- Podcast Split Screen
- Dead Air Cut
- Studio Audio
- Punch Zoom
- Captions
- Social Publishing Kit
- Project management
- Existing rendering pipeline

Podcast should be an additional processing mode.

================================================== 16. PUNCH ZOOM / DEAD AIR / STUDIO AUDIO
==================================================

Keep:

Punch Zoom
Dead Air Cut
Studio Audio

as independent features.

They must not be tightly coupled to the number-of-people layout.

Punch Zoom should remain usable with:

- Single-person layout
- Two-person layout
- Three-person layout

================================================== 17. FILES TO INSPECT AND UPDATE
==================================================

Inspect the existing implementation first.

Pay particular attention to:

src/main.tsx

src-tauri/src/media.rs

src-tauri/src/models.rs

src-tauri/src/lib.rs

src-tauri/src/transcription.rs

src-tauri/bin/face_tracker.swift

and any other files currently involved in Podcast Split Screen, face tracking, transcription, or rendering.

Do not blindly overwrite existing code.

Understand the existing flow and extend it cleanly.

================================================== 18. IMPORTANT ARCHITECTURAL CHANGE
==================================================

The current system is essentially built around:

Person 1
Person 2

Change this to:

PersonTrack[]

and:

LayoutSegment[]

This is the core requirement.

The system must become a general multi-person timeline-based system rather than a fixed two-person system.

================================================== 19. TESTING
==================================================

After implementation, test:

1. Video with one person for the entire clip.
2. Video with two people for the entire clip.
3. Video with three people for the entire clip.
4. One person entering a two-person conversation.
5. One person leaving a three-person conversation.
6. Person temporarily disappearing due to occlusion.
7. Person returning after temporary disappearance.
8. Person moving around the frame.
9. People changing who is speaking.
10. Layout transitions:
    1 → 2
    2 → 3
    3 → 2
    2 → 1
11. Captions during all layout changes.
12. Punch Zoom with all layouts.
13. Dead Air Cut with all layouts.
14. Studio Audio with all layouts.
15. Existing Center Crop mode.
16. Existing Original mode.
17. Existing Shorts/Reels workflow.

Check:

- Rust compilation
- TypeScript compilation
- FFmpeg filter generation
- Tauri commands
- Tracking data serialization/deserialization
- Rendering output
- Audio/video synchronization

Fix any errors you encounter.

==================================================
FINAL REQUIREMENT
==================================================

Do not stop after analyzing the repository.

Do not only give me recommendations.

Do not create a mock implementation.

Actually modify the existing ClipOn codebase and implement the feature.

The final Podcast system should behave like this:

16:9 source
↓
Continuously analyze frames
↓
Track persistent people
↓
Determine current people throughout the timeline
↓
1 person → FULL 9:16
↓
2 people → TOP + BOTTOM
↓
3 people → TWO TOP + ONE BOTTOM
↓
People enter/leave → layout changes accordingly
↓
Temporary disappearance → preserve identity
↓
Dynamic face tracking
↓
Same source timestamp for every section
↓
Synchronized captions
↓
Punch Zoom / Dead Air Cut / Studio Audio
↓
Final Instagram/YouTube-ready 9:16 video

Use the provided reference screenshots as the visual reference for the 2-person and 3-person compositions.

Implement this completely in the existing ClipOn architecture.
