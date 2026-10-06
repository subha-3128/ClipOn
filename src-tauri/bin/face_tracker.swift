import Foundation
import Vision
import AppKit
import Accelerate

// =============================================================================
// DynamicPodcastReframing Data Model
// =============================================================================

enum TrackingState: String, Codable {
    case tentative          // First detected, needs enterThreshold confirmations
    case visible            // Actively detected & matched in current frame
    case temporarilyLost    // Missed in frame, within lostTimeout window (holds position)
    case reappeared         // Recovered from temporarilyLost back to visible
    case exited             // Absent for longer than lostTimeout (inactive)
}

struct PersonKeyframe: Codable {
    let t: Double
    let x: Double
    let y: Double
    let width: Double
    let height: Double
    let confidence: Double
    let visible: Bool
    let state: String
}

struct PersonTrack: Codable {
    let id: Int
    let name: String
    let keyframes: [PersonKeyframe]
}

struct LayoutSegment: Codable {
    let start: Double
    let end: Double
    let number_of_people: Int
    let layout_type: String // "single", "split_two", "split_three"
    let person_ids: [Int]
}

struct LegacyPodcastKeyframe: Codable {
    let t: Double
    let x: Double
    let y: Double
}

struct DetectedFace {
    let x: Double
    let y: Double
    let width: Double
    let height: Double
    let embedding: [Float]
    let confidence: Double
}

struct FrameSample {
    let t: Double
    let faces: [DetectedFace]
}

// =============================================================================
// Helper Functions: Binary Lookup & Math
// =============================================================================

func findBinary(name: String) -> String {
    let candidates = [
        "/opt/homebrew/bin/" + name,
        "/usr/local/bin/" + name,
        "/usr/bin/" + name
    ]
    for c in candidates {
        if FileManager.default.isExecutableFile(atPath: c) {
            return c
        }
    }
    return name
}

func cosineSimilarity(_ a: [Float], _ b: [Float]) -> Float {
    guard a.count == b.count, !a.isEmpty else { return 0.0 }
    var dot: Float = 0.0
    var normA: Float = 0.0
    var normB: Float = 0.0
    vDSP_dotpr(a, 1, b, 1, &dot, vDSP_Length(a.count))
    vDSP_svesq(a, 1, &normA, vDSP_Length(a.count))
    vDSP_svesq(b, 1, &normB, vDSP_Length(b.count))
    let denom = sqrt(normA) * sqrt(normB)
    return denom > 0 ? dot / denom : 0.0
}

func extractFaceEmbedding(from observation: VNFaceObservation) -> [Float] {
    var signature: [Float] = []
    if let landmarks = observation.landmarks {
        let allPoints: [CGPoint] = [
            landmarks.leftEye?.normalizedPoints ?? [],
            landmarks.rightEye?.normalizedPoints ?? [],
            landmarks.nose?.normalizedPoints ?? [],
            landmarks.outerLips?.normalizedPoints ?? [],
            landmarks.leftEyebrow?.normalizedPoints ?? [],
            landmarks.rightEyebrow?.normalizedPoints ?? []
        ].flatMap { $0 }
        for pt in allPoints {
            signature.append(Float(pt.x))
            signature.append(Float(pt.y))
        }
    }
    let target = 64
    if signature.count > target {
        signature = Array(signature.prefix(target))
    } else {
        signature.append(contentsOf: Array(repeating: 0.0, count: target - signature.count))
    }
    var norm: Float = 0.0
    vDSP_svesq(signature, 1, &norm, vDSP_Length(signature.count))
    norm = sqrt(norm)
    if norm > 0 {
        var scale = 1.0 / norm
        vDSP_vsmul(signature, 1, &scale, &signature, 1, vDSP_Length(signature.count))
    }
    return signature
}

func extractDenseFrames(videoPath: String, startSec: Double, durationSec: Double, fps: Double, outDir: String) -> [(Double, String)] {
    let fm = FileManager.default
    try? fm.createDirectory(atPath: outDir, withIntermediateDirectories: true, attributes: nil)
    
    let ffmpegPath = findBinary(name: "ffmpeg")
    let outPattern = "\(outDir)/frame_%04d.jpg"
    
    let p = Process()
    p.executableURL = URL(fileURLWithPath: ffmpegPath)
    p.arguments = [
        "-nostdin",
        "-y",
        "-ss", String(format: "%.3f", startSec),
        "-t", String(format: "%.3f", durationSec),
        "-i", videoPath,
        "-vf", String(format: "fps=%.2f", fps),
        outPattern
    ]
    p.standardInput = FileHandle.nullDevice
    p.standardOutput = FileHandle.nullDevice
    p.standardError = FileHandle.nullDevice
    try? p.run()
    p.waitUntilExit()
    
    let files = (try? fm.contentsOfDirectory(atPath: outDir).filter { $0.hasSuffix(".jpg") }.sorted()) ?? []
    var result: [(Double, String)] = []
    let frameInterval = 1.0 / fps
    for (idx, file) in files.enumerated() {
        let t = min(durationSec, Double(idx) * frameInterval)
        result.append((t, "\(outDir)/\(file)"))
    }
    return result
}

// =============================================================================
// PersonTrackInstance with State Machine
// =============================================================================

class PersonTrackInstance {
    let id: Int
    let name: String
    var state: TrackingState = .tentative
    var embedding: [Float]
    var lastKnownX: Double
    var lastKnownY: Double
    var lastKnownW: Double
    var lastKnownH: Double
    var smoothX: Double
    var smoothY: Double
    var lastSeenTime: Double = -999.0
    var firstConfirmedTime: Double = -999.0
    var consecutiveHits: Int = 0
    var consecutiveMisses: Int = 0
    var totalDetections: Int = 0
    var keyframes: [PersonKeyframe] = []

    init(id: Int, name: String, x: Double, y: Double, w: Double, h: Double, emb: [Float], t: Double) {
        self.id = id
        self.name = name
        self.embedding = emb
        self.lastKnownX = x
        self.lastKnownY = y
        self.lastKnownW = w
        self.lastKnownH = h
        self.smoothX = x
        self.smoothY = y
        self.consecutiveHits = 1
        self.totalDetections = 1
        self.lastSeenTime = t
    }

    func updateMatched(face: DetectedFace, t: Double, enterThreshold: Int) {
        consecutiveHits += 1
        consecutiveMisses = 0
        totalDetections += 1
        lastSeenTime = t

        // State transitions
        switch state {
        case .tentative:
            if consecutiveHits >= enterThreshold {
                state = .visible
                if firstConfirmedTime < 0.0 {
                    firstConfirmedTime = t
                }
            }
        case .temporarilyLost:
            state = .reappeared
        case .reappeared, .visible:
            state = .visible
        case .exited:
            if consecutiveHits >= enterThreshold {
                state = .visible
            }
        }

        lastKnownX = face.x
        lastKnownY = face.y
        lastKnownW = face.width
        lastKnownH = face.height

        // Exponential moving average for facial feature embedding
        for i in 0..<embedding.count {
            embedding[i] = 0.92 * embedding[i] + 0.08 * face.embedding[i]
        }

        // Smooth coordinate interpolation (alpha = 0.60)
        let alpha = 0.60
        smoothX = smoothX + alpha * (face.x - smoothX)
        smoothY = smoothY + alpha * (face.y - smoothY)

        keyframes.append(PersonKeyframe(
            t: t,
            x: smoothX,
            y: smoothY,
            width: face.width,
            height: face.height,
            confidence: face.confidence,
            visible: true,
            state: state.rawValue
        ))
    }

    func updateMissed(t: Double, lostTimeout: Double) {
        consecutiveMisses += 1
        consecutiveHits = 0

        let timeSinceSeen = t - lastSeenTime

        // State transitions
        switch state {
        case .visible, .reappeared:
            if timeSinceSeen <= lostTimeout {
                state = .temporarilyLost
            } else {
                state = .exited
            }
        case .temporarilyLost:
            if timeSinceSeen > lostTimeout {
                state = .exited
            }
        case .tentative:
            if timeSinceSeen > 1.5 {
                state = .exited
            }
        case .exited:
            break
        }

        // Hold last valid crop position with slight relaxation damping (alpha = 0.05)
        keyframes.append(PersonKeyframe(
            t: t,
            x: smoothX,
            y: smoothY,
            width: lastKnownW,
            height: lastKnownH,
            confidence: 0.0,
            visible: false,
            state: state.rawValue
        ))
    }

    var isActiveInScene: Bool {
        return state == .visible || state == .temporarilyLost || state == .reappeared
    }
}

// =============================================================================
// DynamicPodcastReframingEngine
// =============================================================================

class DynamicPodcastReframingEngine {
    // Configuration thresholds
    let sampleRateFps: Double = 3.5            // 3.5 samples/sec (285ms intervals)
    let enterThresholdHits: Int = 3            // Require 3 consecutive hits (~0.85s) to confirm entry
    let confidenceThreshold: Double = 0.45     // Minimum face detection confidence
    let lostTimeoutSec: Double = 3.5           // 3.5s hysteresis for temporary occlusion / looking away
    let minLayoutDurationSec: Double = 3.0      // Hysteresis: minimum duration a layout must persist
    let maxSupportedPeople: Int = 3            // Up to 3 active people in 9:16 vertical canvas

    func execute(videoPath: String, startSec: Double, durationSec: Double) -> String {
        let tempDir = NSTemporaryDirectory() + "clipon_pod_\(UUID().uuidString)"
        defer {
            try? FileManager.default.removeItem(atPath: tempDir)
        }

        // Step 1: High-density temporal sampling (3.5 samples/sec)
        let frameList = extractDenseFrames(
            videoPath: videoPath,
            startSec: startSec,
            durationSec: durationSec,
            fps: sampleRateFps,
            outDir: tempDir
        )

        var samples: [FrameSample] = []
        var allFaceCenters: [Double] = []

        for (t, filePath) in frameList {
            guard let img = NSImage(contentsOfFile: filePath),
                  let cg = img.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
                samples.append(FrameSample(t: t, faces: []))
                continue
            }

            let req = VNDetectFaceLandmarksRequest()
            let handler = VNImageRequestHandler(cgImage: cg, options: [:])
            try? handler.perform([req])

            var detected: [DetectedFace] = []
            for face in req.results ?? [] {
                if Double(face.confidence) < confidenceThreshold { continue }
                let x = face.boundingBox.midX
                let y = 1.0 - face.boundingBox.midY
                let w = face.boundingBox.width
                let h = face.boundingBox.height
                let emb = extractFaceEmbedding(from: face)
                let conf = Double(face.confidence)
                detected.append(DetectedFace(x: x, y: y, width: w, height: h, embedding: emb, confidence: conf))
                allFaceCenters.append(x)
            }
            samples.append(FrameSample(t: t, faces: detected))
        }

        // Step 2: Temporal Identity Discovery & Tracking
        // Persistent identities: Track 1 = Person 1, Track 2 = Person 2, Track 3 = Person 3
        // Discovered chronologically through face embedding similarity.
        // Screen position (left/right) does NOT define or reorder person IDs!
        var personTracks: [PersonTrackInstance] = []

        struct FrameStateRecord {
            let t: Double
            let activePersonIds: [Int]
        }

        var frameRecords: [FrameStateRecord] = []

        for sample in samples {
            let t = sample.t
            var assignedFaceIndices = Set<Int>()

            // A. Match existing tracks to detected faces via appearance embeddings
            for track in personTracks {
                var bestFaceIdx = -1
                var bestSim: Float = 0.38 // Similarity threshold

                for (fIdx, face) in sample.faces.enumerated() {
                    if assignedFaceIndices.contains(fIdx) { continue }
                    let sim = cosineSimilarity(face.embedding, track.embedding)
                    if sim > bestSim {
                        bestSim = sim
                        bestFaceIdx = fIdx
                    }
                }

                if bestFaceIdx != -1 {
                    assignedFaceIndices.insert(bestFaceIdx)
                    track.updateMatched(face: sample.faces[bestFaceIdx], t: t, enterThreshold: enterThresholdHits)
                } else {
                    track.updateMissed(t: t, lostTimeout: lostTimeoutSec)
                }
            }

            // B. Unmatched faces discover new persistent individuals (up to maxSupportedPeople)
            if sample.faces.count > assignedFaceIndices.count && personTracks.count < maxSupportedPeople {
                for (fIdx, face) in sample.faces.enumerated() {
                    if !assignedFaceIndices.contains(fIdx) && personTracks.count < maxSupportedPeople {
                        let newId = personTracks.count + 1
                        let newTrack = PersonTrackInstance(
                            id: newId,
                            name: "Person \(newId)",
                            x: face.x,
                            y: face.y,
                            w: face.width,
                            h: face.height,
                            emb: face.embedding,
                            t: t
                        )
                        personTracks.append(newTrack)
                        assignedFaceIndices.insert(fIdx)
                    }
                }
            }

            // Collect active people in this frame
            let activeIds = personTracks.filter { $0.isActiveInScene }.map { $0.id }
            frameRecords.append(FrameStateRecord(t: t, activePersonIds: activeIds))
        }

        // Filter valid tracks (must have had at least enterThresholdHits or be the sole detected track)
        let confirmedTracks = personTracks.filter {
            $0.totalDetections >= enterThresholdHits || personTracks.count == 1
        }

        // Clamp early entries (< 2.5s) to t=0.0
        for track in confirmedTracks {
            if track.firstConfirmedTime >= 0.0 && track.firstConfirmedTime <= 2.5 {
                track.firstConfirmedTime = 0.0
            }
        }

        // Step 3: Layout State Machine with Minimum Layout Duration Hysteresis
        var rawSegments: [LayoutSegment] = []

        if !frameRecords.isEmpty {
            var currentLayout = "single"
            var currentPersonIds: [Int] = [1]
            var currentStart = 0.0

            let determineLayoutType = { (ids: [Int]) -> String in
                switch ids.count {
                case 1: return "single"
                case 2: return "split_two"
                case 3: return "split_three"
                default: return ids.count >= 3 ? "split_three" : (ids.count == 2 ? "split_two" : "single")
                }
            }

            // Initialize first frame layout
            let firstIds = frameRecords[0].activePersonIds.filter { id in confirmedTracks.contains(where: { $0.id == id }) }
            if !firstIds.isEmpty {
                currentPersonIds = firstIds
                currentLayout = determineLayoutType(currentPersonIds)
            } else if let firstTrack = confirmedTracks.first {
                currentPersonIds = [firstTrack.id]
                currentLayout = "single"
            }

            for rec in frameRecords {
                let validIds = rec.activePersonIds.filter { id in confirmedTracks.contains(where: { $0.id == id }) }
                let targetIds = validIds.isEmpty ? currentPersonIds : validIds
                let targetLayout = determineLayoutType(targetIds)

                let elapsedInLayout = rec.t - currentStart

                // Layout transition triggers only when new state persists and min duration met
                if (targetLayout != currentLayout || targetIds != currentPersonIds) && elapsedInLayout >= minLayoutDurationSec {
                    rawSegments.append(LayoutSegment(
                        start: currentStart,
                        end: rec.t,
                        number_of_people: currentPersonIds.count,
                        layout_type: currentLayout,
                        person_ids: currentPersonIds
                    ))
                    currentStart = rec.t
                    currentLayout = targetLayout
                    currentPersonIds = targetIds
                }
            }

            rawSegments.append(LayoutSegment(
                start: currentStart,
                end: durationSec,
                number_of_people: currentPersonIds.count,
                layout_type: currentLayout,
                person_ids: currentPersonIds
            ))
        }

        // Step 4: Stabilize Segments (merge any residual segment < minLayoutDurationSec)
        var stabilizedSegments: [LayoutSegment] = []
        for seg in rawSegments {
            let dur = seg.end - seg.start
            if dur < minLayoutDurationSec && !stabilizedSegments.isEmpty {
                let prev = stabilizedSegments.removeLast()
                stabilizedSegments.append(LayoutSegment(
                    start: prev.start,
                    end: seg.end,
                    number_of_people: prev.number_of_people,
                    layout_type: prev.layout_type,
                    person_ids: prev.person_ids
                ))
            } else {
                stabilizedSegments.append(seg)
            }
        }

        if stabilizedSegments.isEmpty {
            let count = min(maxSupportedPeople, max(1, confirmedTracks.count))
            let type = count == 1 ? "single" : (count == 2 ? "split_two" : "split_three")
            let ids = Array(confirmedTracks.prefix(count).map { $0.id })
            stabilizedSegments.append(LayoutSegment(
                start: 0.0,
                end: durationSec,
                number_of_people: count,
                layout_type: type,
                person_ids: ids.isEmpty ? [1] : ids
            ))
        }

        // Step 5: JSON Output Generation
        let encoder = JSONEncoder()

        let tracksOutput: [PersonTrack] = confirmedTracks.map { track in
            PersonTrack(id: track.id, name: track.name, keyframes: track.keyframes)
        }

        let peopleJson = (try? String(data: encoder.encode(tracksOutput), encoding: .utf8)) ?? "[]"
        let segmentsJson = (try? String(data: encoder.encode(stabilizedSegments), encoding: .utf8)) ?? "[]"

        let p1Kfs: [LegacyPodcastKeyframe] = (confirmedTracks.first?.keyframes ?? []).map {
            LegacyPodcastKeyframe(t: $0.t, x: $0.x, y: $0.y)
        }
        let p2Kfs: [LegacyPodcastKeyframe] = (confirmedTracks.count > 1 ? confirmedTracks[1].keyframes : []).map {
            LegacyPodcastKeyframe(t: $0.t, x: $0.x, y: $0.y)
        }

        let p1Json = (try? String(data: encoder.encode(p1Kfs), encoding: .utf8)) ?? "[]"
        let p2Json = (try? String(data: encoder.encode(p2Kfs), encoding: .utf8)) ?? "[]"

        let avgCenterX = allFaceCenters.isEmpty ? 0.5 : (allFaceCenters.reduce(0, +) / Double(allFaceCenters.count))
        let p1HomeX = confirmedTracks.first?.lastKnownX ?? 0.26
        let p1HomeY = confirmedTracks.first?.lastKnownY ?? 0.38
        let p2HomeX = confirmedTracks.count > 1 ? confirmedTracks[1].lastKnownX : 0.78
        let p2HomeY = confirmedTracks.count > 1 ? confirmedTracks[1].lastKnownY : 0.38
        let twoFacesDetected = confirmedTracks.count >= 2

        return String(
            format: "{\"avg_center_x\": %.3f, \"face_detected\": %@, \"width\": 1920, \"height\": 1080, \"podcast\": {\"top_center_x\": %.3f, \"top_center_y\": %.3f, \"bottom_center_x\": %.3f, \"bottom_center_y\": %.3f, \"two_faces_detected\": %@, \"person_1_keyframes\": %@, \"person_2_keyframes\": %@, \"people\": %@, \"segments\": %@}}",
            avgCenterX,
            allFaceCenters.isEmpty ? "false" : "true",
            p1HomeX, p1HomeY,
            p2HomeX, p2HomeY,
            twoFacesDetected ? "true" : "false",
            p1Json,
            p2Json,
            peopleJson,
            segmentsJson
        )
    }
}

// Entry Point
let args = CommandLine.arguments
if args.count < 4 {
    print("{\"avg_center_x\": 0.5, \"face_detected\": false, \"podcast\": {\"top_center_x\": 0.26, \"top_center_y\": 0.38, \"bottom_center_x\": 0.78, \"bottom_center_y\": 0.38, \"two_faces_detected\": false, \"people\": [], \"segments\": []}}")
    exit(0)
}

let videoPath = args[1]
let startSec = Double(args[2]) ?? 0.0
let durationSec = Double(args[3]) ?? 1.0

let engine = DynamicPodcastReframingEngine()
let output = engine.execute(videoPath: videoPath, startSec: startSec, durationSec: durationSec)
print(output)