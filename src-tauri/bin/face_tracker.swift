import Foundation
import Vision
import AppKit
import Accelerate

struct PersonKeyframe: Codable {
    let t: Double
    let x: Double
    let y: Double
    let width: Double
    let height: Double
    let confidence: Double
    let visible: Bool
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

// Backward compatibility keyframe struct
struct LegacyPodcastKeyframe: Codable {
    let t: Double
    let x: Double
    let y: Double
}

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

func extractFrames(videoPath: String, startSec: Double, durationSec: Double, count: Int, outDir: String) -> [String] {
    let fm = FileManager.default
    try? fm.createDirectory(atPath: outDir, withIntermediateDirectories: true, attributes: nil)
    
    let ffmpegPath = findBinary(name: "ffmpeg")
    let fpsStr = String(format: "%.4f", Double(count) / max(1.0, durationSec))
    let outPattern = "\(outDir)/frame_%03d.jpg"
    
    let p = Process()
    p.executableURL = URL(fileURLWithPath: ffmpegPath)
    p.arguments = [
        "-nostdin",
        "-y",
        "-ss", String(format: "%.3f", startSec),
        "-t", String(format: "%.3f", durationSec),
        "-i", videoPath,
        "-vf", "fps=\(fpsStr)",
        "-vframes", "\(count)",
        outPattern
    ]
    p.standardInput = FileHandle.nullDevice
    p.standardOutput = FileHandle.nullDevice
    p.standardError = FileHandle.nullDevice
    try? p.run()
    p.waitUntilExit()
    
    let files = (try? fm.contentsOfDirectory(atPath: outDir).filter { $0.hasSuffix(".jpg") }.sorted()) ?? []
    return files.map { "\(outDir)/\($0)" }
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

struct DetectedFace {
    let x: Double
    let y: Double
    let width: Double
    let height: Double
    let yaw: Double
    let embedding: [Float]
    let confidence: Double
}

struct FrameSample {
    let t: Double
    let faces: [DetectedFace]
}

class KnownPerson {
    let id: Int
    var name: String
    var homeX: Double
    var homeY: Double
    var embedding: [Float]
    var lastKnownX: Double
    var lastKnownY: Double
    var lastKnownW: Double
    var lastKnownH: Double
    var lastSeenTime: Double = -999.0
    var firstSeenTime: Double = -999.0
    var detectionCount: Int = 0
    var smoothX: Double
    var smoothY: Double
    var keyframes: [PersonKeyframe] = []
    
    init(id: Int, name: String, x: Double, y: Double, w: Double, h: Double, emb: [Float], t: Double) {
        self.id = id
        self.name = name
        self.homeX = x
        self.homeY = y
        self.embedding = emb
        self.lastKnownX = x
        self.lastKnownY = y
        self.lastKnownW = w
        self.lastKnownH = h
        self.smoothX = x
        self.smoothY = y
        self.lastSeenTime = -999.0
        self.firstSeenTime = -999.0
        self.detectionCount = 0
    }
}

func runTracker() {
    let args = CommandLine.arguments
    guard args.count >= 4 else {
        print("{\"avg_center_x\": 0.5, \"face_detected\": false, \"podcast\": {\"top_center_x\": 0.26, \"top_center_y\": 0.38, \"bottom_center_x\": 0.78, \"bottom_center_y\": 0.38, \"two_faces_detected\": false, \"people\": [], \"segments\": []}}")
        return
    }

    let videoPath = args[1]
    let startSec = Double(args[2]) ?? 0.0
    let durationSec = Double(args[3]) ?? 1.0

    let tempDir = NSTemporaryDirectory() + "clipon_trk_\(UUID().uuidString)"
    defer {
        try? FileManager.default.removeItem(atPath: tempDir)
    }

    // High frequency sampling (~1.0s interval, min 15, max 90 frames)
    let step = 1.0
    let frameCount = max(15, min(90, Int(ceil(durationSec / step)) + 1))
    let frameFiles = extractFrames(videoPath: videoPath, startSec: startSec, durationSec: durationSec, count: frameCount, outDir: tempDir)

    var samples: [FrameSample] = []
    var allFaceCenters: [Double] = []

    for (idx, filePath) in frameFiles.enumerated() {
        let t = Double(idx) * (durationSec / Double(max(1, frameFiles.count - 1)))
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
            let x = face.boundingBox.midX
            let y = 1.0 - face.boundingBox.midY
            let w = face.boundingBox.width
            let h = face.boundingBox.height
            let yaw = face.yaw?.doubleValue ?? 0.0
            let emb = extractFaceEmbedding(from: face)
            let conf = Double(face.confidence)
            detected.append(DetectedFace(x: x, y: y, width: w, height: h, yaw: yaw, embedding: emb, confidence: conf))
            allFaceCenters.append(x)
        }
        // Sort left to right
        detected.sort { $0.x < $1.x }
        samples.append(FrameSample(t: t, faces: detected))
    }

    // =========================================================================
    // PHASE 1: Discover Persistent Identities
    // =========================================================================
    var knownPersons: [KnownPerson] = []

    // 1. Check for multi-face wide shots with distinct separated faces
    let multiSamples = samples.filter { sample in
        if sample.faces.count >= 2 {
            // Ensure faces are well-separated
            let sep = sample.faces.last!.x - sample.faces.first!.x
            return sep > 0.12
        }
        return false
    }

    if let bestMulti = multiSamples.max(by: { $0.faces.count < $1.faces.count }) {
        // We have a wide shot with 2 or 3+ people
        for (idx, face) in bestMulti.faces.prefix(3).enumerated() {
            let person = KnownPerson(
                id: idx + 1,
                name: "Person \(idx + 1)",
                x: face.x,
                y: face.y,
                w: face.width,
                h: face.height,
                emb: face.embedding,
                t: 0.0
            )
            knownPersons.append(person)
        }
    } else {
        // Check single faces: distinguish Person 1 and Person 2 using yaw and X
        var p1Faces: [DetectedFace] = []
        var p2Faces: [DetectedFace] = []
        for s in samples {
            for f in s.faces {
                if f.yaw > 0.10 || (f.yaw >= -0.05 && f.x < 0.45) {
                    p1Faces.append(f)
                } else if f.yaw < -0.10 || (f.yaw <= 0.05 && f.x > 0.55) {
                    p2Faces.append(f)
                }
            }
        }
        if !p1Faces.isEmpty {
            let f = p1Faces.first!
            let avgX = min(0.40, p1Faces.map { $0.x }.reduce(0, +) / Double(p1Faces.count))
            let avgY = p1Faces.map { $0.y }.reduce(0, +) / Double(p1Faces.count)
            knownPersons.append(KnownPerson(id: 1, name: "Person 1", x: avgX, y: avgY, w: f.width, h: f.height, emb: f.embedding, t: 0.0))
        }
        if !p2Faces.isEmpty {
            let f = p2Faces.first!
            let avgX = max(0.60, p2Faces.map { $0.x }.reduce(0, +) / Double(p2Faces.count))
            let avgY = p2Faces.map { $0.y }.reduce(0, +) / Double(p2Faces.count)
            knownPersons.append(KnownPerson(id: 2, name: "Person 2", x: avgX, y: avgY, w: f.width, h: f.height, emb: f.embedding, t: 0.0))
        }
    }

    if knownPersons.isEmpty {
        // Fallback single person centered
        let firstFace = samples.flatMap { $0.faces }.first
        let x = firstFace?.x ?? 0.50
        let y = firstFace?.y ?? 0.38
        let emb = firstFace?.embedding ?? Array(repeating: 0.0, count: 64)
        knownPersons.append(KnownPerson(id: 1, name: "Person 1", x: x, y: y, w: 0.2, h: 0.2, emb: emb, t: 0.0))
    }

    // Ensure sorted left-to-right home positions
    knownPersons.sort { $0.homeX < $1.homeX }
    for (i, p) in knownPersons.enumerated() {
        p.name = "Person \(i + 1)"
    }

    // =========================================================================
    // PHASE 2: Dynamic Per-Frame Tracking with Identity Locking
    // =========================================================================
    struct SampleMatchRecord {
        let t: Double
        let matchedPersonIds: Set<Int>
    }

    var sampleMatches: [SampleMatchRecord] = []

    for sample in samples {
        let t = sample.t
        var assignedFaces = Set<Int>()
        var matchedPersonIds = Set<Int>()

        // 1. Match each known person to detected faces
        for person in knownPersons {
            var bestIdx = -1
            var bestScore: Float = -1.0
            for (fIdx, face) in sample.faces.enumerated() {
                if assignedFaces.contains(fIdx) { continue }
                let sim = cosineSimilarity(face.embedding, person.embedding)
                let refX = person.detectionCount > 0 ? person.lastKnownX : person.homeX
                let refY = person.detectionCount > 0 ? person.lastKnownY : person.homeY
                let dist = sqrt(pow(face.x - refX, 2) + pow(face.y - refY, 2))
                let score = sim - Float(dist * 0.35)
                if score > bestScore && sim >= 0.38 {
                    bestScore = score
                    bestIdx = fIdx
                }
            }

            if bestIdx != -1 {
                // Confirmed match
                let face = sample.faces[bestIdx]
                assignedFaces.insert(bestIdx)
                matchedPersonIds.insert(person.id)

                person.lastKnownX = face.x
                person.lastKnownY = face.y
                person.lastKnownW = face.width
                person.lastKnownH = face.height
                person.lastSeenTime = t
                if person.firstSeenTime < 0.0 {
                    person.firstSeenTime = t
                }
                person.detectionCount += 1

                // EMA update embedding
                for j in 0..<person.embedding.count {
                    person.embedding[j] = 0.90 * person.embedding[j] + 0.10 * face.embedding[j]
                }

                // Smooth coordinate transition
                let alpha = 0.65
                person.smoothX = person.smoothX + alpha * (face.x - person.smoothX)
                person.smoothY = person.smoothY + alpha * (face.y - person.smoothY)

                person.keyframes.append(PersonKeyframe(
                    t: t,
                    x: person.smoothX,
                    y: person.smoothY,
                    width: face.width,
                    height: face.height,
                    confidence: face.confidence,
                    visible: true
                ))
            } else {
                // Temporarily disappeared/absent: HOLD last valid or home position
                let targetX = person.detectionCount > 0 ? person.lastKnownX : person.homeX
                let targetY = person.detectionCount > 0 ? person.lastKnownY : person.homeY
                let alpha = 0.30
                person.smoothX = person.smoothX + alpha * (targetX - person.smoothX)
                person.smoothY = person.smoothY + alpha * (targetY - person.smoothY)

                person.keyframes.append(PersonKeyframe(
                    t: t,
                    x: person.smoothX,
                    y: person.smoothY,
                    width: person.lastKnownW,
                    height: person.lastKnownH,
                    confidence: 0.0,
                    visible: false
                ))
            }
        }

        // 2. Discover new persons appearing later in video (up to 3 total)
        if sample.faces.count > assignedFaces.count && knownPersons.count < 3 {
            for (fIdx, face) in sample.faces.enumerated() {
                if !assignedFaces.contains(fIdx) {
                    let newId = knownPersons.count + 1
                    let newP = KnownPerson(
                        id: newId,
                        name: "Person \(newId)",
                        x: face.x,
                        y: face.y,
                        w: face.width,
                        h: face.height,
                        emb: face.embedding,
                        t: t
                    )
                    newP.smoothX = face.x
                    newP.smoothY = face.y
                    newP.firstSeenTime = t
                    newP.lastSeenTime = t
                    newP.detectionCount = 1
                    newP.keyframes.append(PersonKeyframe(
                        t: t,
                        x: face.x,
                        y: face.y,
                        width: face.width,
                        height: face.height,
                        confidence: face.confidence,
                        visible: true
                    ))
                    knownPersons.append(newP)
                    assignedFaces.insert(fIdx)
                    matchedPersonIds.insert(newId)
                }
            }
        }

        sampleMatches.append(SampleMatchRecord(t: t, matchedPersonIds: matchedPersonIds))
    }

    // Filter confirmed persons: ignore 1-frame transient noise unless only 1 person exists
    let validPersons = knownPersons.filter { p in
        p.detectionCount >= 2 || knownPersons.count == 1 || p.detectionCount == (knownPersons.map { $0.detectionCount }.max() ?? 1)
    }

    // Clamp early entries (< 3.0s) to clip start and late exits to clip end
    for p in validPersons {
        if p.firstSeenTime >= 0.0 && p.firstSeenTime <= 3.0 {
            p.firstSeenTime = 0.0
        }
        if p.lastSeenTime >= (durationSec - 3.0) {
            p.lastSeenTime = durationSec
        }
    }

    struct FrameLayoutRecord {
        let t: Double
        let layoutType: String
        let activePersonIds: [Int]
    }

    var frameRecords: [FrameLayoutRecord] = []

    for match in sampleMatches {
        let t = match.t
        let activePersons = validPersons.filter { p in
            guard p.firstSeenTime >= 0.0 && t >= p.firstSeenTime else { return false }
            let timeSinceSeen = t - p.lastSeenTime
            return match.matchedPersonIds.contains(p.id) || timeSinceSeen <= 3.5 || t <= p.lastSeenTime
        }

        let layoutType: String
        let activeIds: [Int]
        if activePersons.count == 1 {
            layoutType = "single"
            activeIds = [activePersons[0].id]
        } else if activePersons.count == 2 {
            layoutType = "split_two"
            activeIds = [activePersons[0].id, activePersons[1].id]
        } else if activePersons.count >= 3 {
            layoutType = "split_three"
            activeIds = Array(activePersons.prefix(3).map { $0.id })
        } else {
            layoutType = "single"
            activeIds = [validPersons.first?.id ?? 1]
        }

        frameRecords.append(FrameLayoutRecord(t: t, layoutType: layoutType, activePersonIds: activeIds))
    }

    // =========================================================================
    // PHASE 3: Build & Stabilize Layout Timeline Segments
    // =========================================================================
    var rawSegments: [LayoutSegment] = []
    if !frameRecords.isEmpty {
        var currType = frameRecords[0].layoutType
        var currIds = frameRecords[0].activePersonIds
        var currStart = 0.0

        for i in 1..<frameRecords.count {
            let rec = frameRecords[i]
            if rec.layoutType != currType || rec.activePersonIds != currIds {
                rawSegments.append(LayoutSegment(
                    start: currStart,
                    end: rec.t,
                    number_of_people: currIds.count,
                    layout_type: currType,
                    person_ids: currIds
                ))
                currStart = rec.t
                currType = rec.layoutType
                currIds = rec.activePersonIds
            }
        }
        rawSegments.append(LayoutSegment(
            start: currStart,
            end: durationSec,
            number_of_people: currIds.count,
            layout_type: currType,
            person_ids: currIds
        ))
    }

    // Stabilize segments (merge short glitches < 2.5s into neighboring segment)
    var stabilizedSegments: [LayoutSegment] = []
    for seg in rawSegments {
        let dur = seg.end - seg.start
        if dur < 2.5 && !stabilizedSegments.isEmpty {
            // Merge with previous segment
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
        let count = min(3, max(1, knownPersons.count))
        let type = count == 1 ? "single" : (count == 2 ? "split_two" : "split_three")
        let ids = Array(knownPersons.prefix(count).map { $0.id })
        stabilizedSegments.append(LayoutSegment(
            start: 0.0,
            end: durationSec,
            number_of_people: count,
            layout_type: type,
            person_ids: ids
        ))
    }

    // =========================================================================
    // PHASE 4: Format JSON Response
    // =========================================================================
    let encoder = JSONEncoder()

    // Export PersonTrack collection
    let personTracks: [PersonTrack] = knownPersons.map { p in
        PersonTrack(id: p.id, name: p.name, keyframes: p.keyframes)
    }

    let peopleJson = (try? String(data: encoder.encode(personTracks), encoding: .utf8)) ?? "[]"
    let segmentsJson = (try? String(data: encoder.encode(stabilizedSegments), encoding: .utf8)) ?? "[]"

    // Backward compatibility keyframes
    let p1Kfs: [LegacyPodcastKeyframe] = (knownPersons.first?.keyframes ?? []).map {
        LegacyPodcastKeyframe(t: $0.t, x: $0.x, y: $0.y)
    }
    let p2Kfs: [LegacyPodcastKeyframe] = (knownPersons.count > 1 ? knownPersons[1].keyframes : []).map {
        LegacyPodcastKeyframe(t: $0.t, x: $0.x, y: $0.y)
    }

    let p1Json = (try? String(data: encoder.encode(p1Kfs), encoding: .utf8)) ?? "[]"
    let p2Json = (try? String(data: encoder.encode(p2Kfs), encoding: .utf8)) ?? "[]"

    let avgCenterX = allFaceCenters.isEmpty ? 0.5 : (allFaceCenters.reduce(0, +) / Double(allFaceCenters.count))
    let p1HomeX = knownPersons.first?.homeX ?? 0.26
    let p1HomeY = knownPersons.first?.homeY ?? 0.38
    let p2HomeX = knownPersons.count > 1 ? knownPersons[1].homeX : 0.78
    let p2HomeY = knownPersons.count > 1 ? knownPersons[1].homeY : 0.38
    let twoFacesDetected = knownPersons.count >= 2

    print(String(
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
    ))
}

runTracker()