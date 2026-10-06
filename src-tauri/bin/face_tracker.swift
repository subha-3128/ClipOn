import Foundation
import Vision
import AppKit
import Accelerate

struct PodcastKeyframe: Codable {
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
    let yaw: Double
    let embedding: [Float]
}

struct FrameSample {
    let t: Double
    let faces: [DetectedFace]
}

func runTracker() {
    let args = CommandLine.arguments
    guard args.count >= 4 else {
        print("{\"avg_center_x\": 0.5, \"face_detected\": false, \"podcast\": {\"top_center_x\": 0.26, \"top_center_y\": 0.38, \"bottom_center_x\": 0.78, \"bottom_center_y\": 0.38, \"two_faces_detected\": false}}")
        return
    }

    let videoPath = args[1]
    let startSec = Double(args[2]) ?? 0.0
    let durationSec = Double(args[3]) ?? 1.0

    let tempDir = NSTemporaryDirectory() + "clipon_trk_\(UUID().uuidString)"
    defer {
        try? FileManager.default.removeItem(atPath: tempDir)
    }

    // Sample every ~1.5s (minimum 12, max 60 frames)
    let step = 1.5
    let frameCount = max(12, min(60, Int(ceil(durationSec / step)) + 1))
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
            let yaw = face.yaw?.doubleValue ?? 0.0
            let emb = extractFaceEmbedding(from: face)
            detected.append(DetectedFace(x: x, y: y, yaw: yaw, embedding: emb))
            allFaceCenters.append(x)
        }
        // Sort left to right
        detected.sort { $0.x < $1.x }
        samples.append(FrameSample(t: t, faces: detected))
    }

    // =========================================================================
    // PHASE 1: Establish Two Distinct Identities & Home Positions
    // =========================================================================
    // Wide shot anchor: frames with 2+ faces separated by at least 15% frame width
    let wideSamples = samples.filter { sample in
        if sample.faces.count >= 2 {
            let sep = sample.faces.last!.x - sample.faces.first!.x
            return sep > 0.15
        }
        return false
    }

    var p1HomeX = 0.26
    var p1HomeY = 0.38
    var p2HomeX = 0.78
    var p2HomeY = 0.38
    var p1RefEmb: [Float]? = nil
    var p2RefEmb: [Float]? = nil
    var twoFacesDetected = false

    if !wideSamples.isEmpty {
        twoFacesDetected = true
        let p1Xs = wideSamples.map { $0.faces.first!.x }
        let p1Ys = wideSamples.map { $0.faces.first!.y }
        let p2Xs = wideSamples.map { $0.faces.last!.x }
        let p2Ys = wideSamples.map { $0.faces.last!.y }

        p1HomeX = p1Xs.reduce(0, +) / Double(p1Xs.count)
        p1HomeY = p1Ys.reduce(0, +) / Double(p1Ys.count)
        p2HomeX = p2Xs.reduce(0, +) / Double(p2Xs.count)
        p2HomeY = p2Ys.reduce(0, +) / Double(p2Ys.count)

        p1RefEmb = wideSamples.first!.faces.first!.embedding
        p2RefEmb = wideSamples.first!.faces.last!.embedding
    } else {
        // Fallback when no wide shots exist in this clip:
        // Use yaw (Host looking right > 0, Guest looking left < 0) and X position
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
            p1RefEmb = p1Faces.first!.embedding
            p1HomeX = min(0.40, p1Faces.map { $0.x }.reduce(0, +) / Double(p1Faces.count))
            p1HomeY = p1Faces.map { $0.y }.reduce(0, +) / Double(p1Faces.count)
        }
        if !p2Faces.isEmpty {
            p2RefEmb = p2Faces.first!.embedding
            p2HomeX = max(0.60, p2Faces.map { $0.x }.reduce(0, +) / Double(p2Faces.count))
            p2HomeY = p2Faces.map { $0.y }.reduce(0, +) / Double(p2Faces.count)
        }
        twoFacesDetected = !p1Faces.isEmpty && !p2Faces.isEmpty
    }

    // =========================================================================
    // PHASE 2: Dynamic Per-Segment Tracking
    // =========================================================================
    // Person 1 is permanently locked to Top section.
    // Person 2 is permanently locked to Bottom section.
    // When a person temporarily disappears, hold their home/last position.
    // Neither person is EVER assigned to the other section.

    var p1Keyframes: [PodcastKeyframe] = []
    var p2Keyframes: [PodcastKeyframe] = []

    var lastP1X = p1HomeX
    var lastP1Y = p1HomeY
    var lastP2X = p2HomeX
    var lastP2Y = p2HomeY

    for sample in samples {
        let t = sample.t
        var targetP1X = p1HomeX
        var targetP1Y = p1HomeY
        var targetP2X = p2HomeX
        var targetP2Y = p2HomeY

        if sample.faces.count >= 2 {
            // Wide shot: Left face -> Person 1 (Top), Right face -> Person 2 (Bottom)
            let f1 = sample.faces.first!
            let f2 = sample.faces.last!
            targetP1X = f1.x
            targetP1Y = f1.y
            targetP2X = f2.x
            targetP2Y = f2.y
        } else if sample.faces.count == 1 {
            let f = sample.faces.first!
            var isP1 = false

            if let emb1 = p1RefEmb, let emb2 = p2RefEmb {
                let s1 = cosineSimilarity(f.embedding, emb1)
                let s2 = cosineSimilarity(f.embedding, emb2)
                if s1 > s2 + 0.08 {
                    isP1 = true
                } else if s2 > s1 + 0.08 {
                    isP1 = false
                } else {
                    isP1 = (f.yaw >= 0.0)
                }
            } else {
                isP1 = (f.yaw >= 0.0)
            }

            if isP1 {
                // Person 1 visible -> Top tracks Person 1
                targetP1X = f.x
                targetP1Y = f.y
                // Person 2 absent -> Bottom holds Person 2 home position
                // Guarantees Person 1 is NEVER shown in Bottom section!
                targetP2X = p2HomeX
                targetP2Y = p2HomeY
            } else {
                // Person 2 visible -> Bottom tracks Person 2
                targetP2X = f.x
                targetP2Y = f.y
                // Person 1 absent -> Top holds Person 1 home position
                // Guarantees Person 2 is NEVER shown in Top section!
                targetP1X = p1HomeX
                targetP1Y = p1HomeY
            }
        } else {
            // No faces in frame -> both hold home positions
            targetP1X = p1HomeX
            targetP1Y = p1HomeY
            targetP2X = p2HomeX
            targetP2Y = p2HomeY
        }

        // Apply smooth exponential moving average to eliminate micro-jitter
        let alpha = 0.65
        let smoothP1X = lastP1X + alpha * (targetP1X - lastP1X)
        let smoothP1Y = lastP1Y + alpha * (targetP1Y - lastP1Y)
        let smoothP2X = lastP2X + alpha * (targetP2X - lastP2X)
        let smoothP2Y = lastP2Y + alpha * (targetP2Y - lastP2Y)

        lastP1X = smoothP1X
        lastP1Y = smoothP1Y
        lastP2X = smoothP2X
        lastP2Y = smoothP2Y

        p1Keyframes.append(PodcastKeyframe(t: t, x: smoothP1X, y: smoothP1Y))
        p2Keyframes.append(PodcastKeyframe(t: t, x: smoothP2X, y: smoothP2Y))
    }

    // Serialize keyframes to JSON
    let encoder = JSONEncoder()
    let p1Json = (try? String(data: encoder.encode(p1Keyframes), encoding: .utf8)) ?? "[]"
    let p2Json = (try? String(data: encoder.encode(p2Keyframes), encoding: .utf8)) ?? "[]"

    let avgCenterX = allFaceCenters.isEmpty ? 0.5 : (allFaceCenters.reduce(0, +) / Double(allFaceCenters.count))

    print(String(
        format: "{\"avg_center_x\": %.3f, \"face_detected\": %@, \"width\": 1920, \"height\": 1080, \"podcast\": {\"top_center_x\": %.3f, \"top_center_y\": %.3f, \"bottom_center_x\": %.3f, \"bottom_center_y\": %.3f, \"two_faces_detected\": %@, \"person_1_keyframes\": %@, \"person_2_keyframes\": %@}}",
        avgCenterX,
        allFaceCenters.isEmpty ? "false" : "true",
        p1HomeX, p1HomeY,
        p2HomeX, p2HomeY,
        twoFacesDetected ? "true" : "false",
        p1Json,
        p2Json
    ))
}

runTracker()