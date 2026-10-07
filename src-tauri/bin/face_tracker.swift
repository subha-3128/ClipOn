import Foundation
import Vision
import AppKit
import Accelerate

// =============================================================================
// Multi-Person Reel Reframing Data Model
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

struct TrackingKeyframeOutput: Codable {
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
    let landmarkEmbedding: [Float]?
    let featurePrint: VNFeaturePrintObservation?
    let confidence: Double
}

struct AppearancePrototype {
    let pixelEmbedding: [Float]
    let landmarkEmbedding: [Float]?
    let featurePrint: VNFeaturePrintObservation?
    let width: Double
    let height: Double
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

func normalizeEmbedding(_ values: [Float]) -> [Float] {
    var signature = values
    let target = 64
    if signature.count > target {
        signature = Array(signature.prefix(target))
    } else if signature.count < target {
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

func extractPixelEmbedding(from image: CGImage, boundingBox: CGRect) -> [Float] {
    let imageWidth = CGFloat(image.width)
    let imageHeight = CGFloat(image.height)
    let clampedBox = boundingBox.intersection(CGRect(x: 0, y: 0, width: 1, height: 1))
    guard !clampedBox.isNull, clampedBox.width > 0, clampedBox.height > 0 else {
        return normalizeEmbedding([])
    }

    let cropRect = CGRect(
        x: clampedBox.minX * imageWidth,
        y: (1.0 - clampedBox.maxY) * imageHeight,
        width: clampedBox.width * imageWidth,
        height: clampedBox.height * imageHeight
    ).integral.intersection(CGRect(x: 0, y: 0, width: imageWidth, height: imageHeight))
    guard let crop = image.cropping(to: cropRect), crop.width > 0, crop.height > 0 else {
        return normalizeEmbedding([])
    }

    var pixels = [UInt8](repeating: 0, count: 4 * 4 * 4)
    let colorSpace = CGColorSpaceCreateDeviceRGB()
    pixels.withUnsafeMutableBytes { buffer in
        guard let baseAddress = buffer.baseAddress,
              let context = CGContext(
                  data: baseAddress,
                  width: 4,
                  height: 4,
                  bitsPerComponent: 8,
                  bytesPerRow: 4 * 4,
                  space: colorSpace,
                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
              ) else { return }
        context.interpolationQuality = .high
        context.draw(crop, in: CGRect(x: 0, y: 0, width: 4, height: 4))
    }
    var signature: [Float] = []
    for pixel in stride(from: 0, to: pixels.count, by: 4) {
        signature.append(Float(pixels[pixel]) / 255.0)
        signature.append(Float(pixels[pixel + 1]) / 255.0)
        signature.append(Float(pixels[pixel + 2]) / 255.0)
    }
    for pixel in stride(from: 0, to: pixels.count, by: 4) {
        let red = Float(pixels[pixel])
        let green = Float(pixels[pixel + 1])
        let blue = Float(pixels[pixel + 2])
        signature.append((0.299 * red + 0.587 * green + 0.114 * blue) / 255.0)
    }
    let mean = signature.reduce(0.0, +) / Float(signature.count)
    return normalizeEmbedding(signature.map { $0 - mean })
}

func extractLandmarkEmbedding(from observation: VNFaceObservation) -> [Float]? {
    var signature: [Float] = []
    guard let landmarks = observation.landmarks else { return nil }
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
    return signature.isEmpty ? nil : normalizeEmbedding(signature)
}

func boundingBoxOverlap(_ lhs: CGRect, _ rhs: CGRect) -> CGFloat {
    let intersection = lhs.intersection(rhs)
    guard !intersection.isNull else { return 0.0 }
    let intersectionArea = intersection.width * intersection.height
    let unionArea = (lhs.width * lhs.height) + (rhs.width * rhs.height) - intersectionArea
    return unionArea > 0 ? intersectionArea / unionArea : 0.0
}

func containsCenter(_ outer: CGRect, _ inner: CGRect) -> Bool {
    outer.contains(CGPoint(x: inner.midX, y: inner.midY))
}

func upperBodyBoundingBox(_ humanBox: CGRect) -> CGRect {
    CGRect(
        x: humanBox.minX,
        y: humanBox.maxY - (humanBox.height * 0.45),
        width: humanBox.width,
        height: humanBox.height * 0.45
    )
}

func makeFeaturePrint(from image: CGImage, boundingBox: CGRect) -> VNFeaturePrintObservation? {
    let imageWidth = CGFloat(image.width)
    let imageHeight = CGFloat(image.height)
    let clampedBox = boundingBox.intersection(CGRect(x: 0, y: 0, width: 1, height: 1))
    guard !clampedBox.isNull, clampedBox.width > 0, clampedBox.height > 0 else { return nil }
    let cropRect = CGRect(
        x: clampedBox.minX * imageWidth,
        y: (1.0 - clampedBox.maxY) * imageHeight,
        width: clampedBox.width * imageWidth,
        height: clampedBox.height * imageHeight
    ).integral.intersection(CGRect(x: 0, y: 0, width: imageWidth, height: imageHeight))
    guard let crop = image.cropping(to: cropRect), crop.width > 0, crop.height > 0 else { return nil }
    let request = VNGenerateImageFeaturePrintRequest()
    let handler = VNImageRequestHandler(cgImage: crop, options: [:])
    try? handler.perform([request])
    return request.results?.first as? VNFeaturePrintObservation
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
    var landmarkEmbedding: [Float]?
    var featurePrint: VNFeaturePrintObservation?
    var lastKnownX: Double
    var lastKnownY: Double
    var lastKnownW: Double
    var lastKnownH: Double
    var smoothX: Double
    var smoothY: Double
    let initialAnchorX: Double
    let initialAnchorY: Double
    var sceneAnchorX: Double
    var sceneAnchorY: Double
    var velocityX: Double = 0.0
    var velocityY: Double = 0.0
    var lastMatchConfidence: Double = 0.0
    var lastSeenTime: Double = -999.0
    var firstConfirmedTime: Double = -999.0
    var consecutiveHits: Int = 0
    var consecutiveMisses: Int = 0
    var totalDetections: Int = 0
    var keyframes: [PersonKeyframe] = []
    var appearanceHistory: [AppearancePrototype]

    init(id: Int, name: String, x: Double, y: Double, w: Double, h: Double, emb: [Float], landmarkEmb: [Float]?, featurePrint: VNFeaturePrintObservation?, t: Double) {
        self.id = id
        self.name = name
        self.embedding = emb
        self.landmarkEmbedding = landmarkEmb
        self.featurePrint = featurePrint
        self.lastKnownX = x
        self.lastKnownY = y
        self.lastKnownW = w
        self.lastKnownH = h
        self.smoothX = x
        self.smoothY = y
        self.initialAnchorX = x
        self.initialAnchorY = y
        self.sceneAnchorX = x
        self.sceneAnchorY = y
        self.consecutiveHits = 1
        self.totalDetections = 1
        self.lastSeenTime = t
        self.lastMatchConfidence = 1.0
        self.appearanceHistory = [AppearancePrototype(
            pixelEmbedding: emb,
            landmarkEmbedding: landmarkEmb,
            featurePrint: featurePrint,
            width: w,
            height: h,
            confidence: 1.0
        )]
    }

    func appearanceScore(for face: DetectedFace) -> Double {
        var bestScore = 0.0
        for prototype in appearanceHistory {
            let pixelSimilarity = max(0.0, min(1.0, (Double(cosineSimilarity(face.embedding, prototype.pixelEmbedding)) + 1.0) / 2.0))
            let landmarkSimilarity: Double
            if let faceLandmarks = face.landmarkEmbedding, let prototypeLandmarks = prototype.landmarkEmbedding {
                landmarkSimilarity = max(0.0, min(1.0, (Double(cosineSimilarity(faceLandmarks, prototypeLandmarks)) + 1.0) / 2.0))
            } else {
                landmarkSimilarity = pixelSimilarity
            }
            var featurePrintSimilarity = pixelSimilarity
            if let candidatePrint = face.featurePrint, let prototypePrint = prototype.featurePrint {
                var distance: Float = 1.0
                if (try? prototypePrint.computeDistance(&distance, to: candidatePrint)) != nil {
                    featurePrintSimilarity = Double(exp(-distance))
                }
            }
            let score = (0.65 * featurePrintSimilarity) + (0.20 * pixelSimilarity) + (0.15 * landmarkSimilarity)
            bestScore = max(bestScore, score)
        }
        return bestScore
    }

    func rememberAppearance(from face: DetectedFace) {
        appearanceHistory.append(AppearancePrototype(
            pixelEmbedding: face.embedding,
            landmarkEmbedding: face.landmarkEmbedding,
            featurePrint: face.featurePrint,
            width: face.width,
            height: face.height,
            confidence: face.confidence
        ))
        if appearanceHistory.count > 8 {
            appearanceHistory.removeFirst()
        }
    }

    func associationScore(face: DetectedFace, t: Double, candidateCount: Int) -> Double {
        let appearanceSimilarity = appearanceScore(for: face)
        let elapsed = max(0.001, t - lastSeenTime)
        let predictedX = lastKnownX + max(-0.25, min(0.25, velocityX * elapsed))
        let predictedY = lastKnownY + max(-0.25, min(0.25, velocityY * elapsed))
        let spatialReferenceX = candidateCount >= 2 ? sceneAnchorX : predictedX
        let spatialReferenceY = candidateCount >= 2 ? sceneAnchorY : predictedY
        let spatialDistance = hypot(face.x - spatialReferenceX, face.y - spatialReferenceY)
        let spatialGate = state == .temporarilyLost ? 0.55 : 0.55
        if spatialDistance > spatialGate {
            return 0.0
        }
        let spatialScore = exp(-spatialDistance / 0.22)
        let observedVelocityX = (face.x - lastKnownX) / elapsed
        let observedVelocityY = (face.y - lastKnownY) / elapsed
        let motionDistance = hypot(observedVelocityX - velocityX, observedVelocityY - velocityY)
        let motionScore = exp(-motionDistance / 0.9)
        let widthRatio = max(0.05, face.width) / max(0.05, lastKnownW)
        let heightRatio = max(0.05, face.height) / max(0.05, lastKnownH)
        let sizeScore = exp(-(abs(log(widthRatio)) + abs(log(heightRatio))) / 0.8)
        let confidenceScore = max(0.0, min(1.0, face.confidence))
        let historyScore = min(1.0, 0.5 + Double(totalDetections) / 20.0)
        let occlusionScore = state == .temporarilyLost || state == .reappeared ? 1.0 : 0.8

        let appearanceWeight = state == .exited ? 0.40 : 0.75
        let spatialWeight = state == .exited ? 0.35 : 0.10
        return (appearanceWeight * appearanceSimilarity)
            + (spatialWeight * spatialScore)
            + (0.05 * motionScore)
            + (0.04 * sizeScore)
            + (0.03 * confidenceScore)
            + (0.03 * historyScore * occlusionScore)
    }

    func reidentificationScore(for face: DetectedFace, candidateCount: Int) -> Double {
        let appearance = appearanceScore(for: face)
        let widthRatio = max(0.05, face.width) / max(0.05, lastKnownW)
        let heightRatio = max(0.05, face.height) / max(0.05, lastKnownH)
        let sizeScore = exp(-(abs(log(widthRatio)) + abs(log(heightRatio))) / 1.2)
        let initialAnchorDistance = hypot(face.x - initialAnchorX, face.y - initialAnchorY)
        let initialAnchorScore = exp(-initialAnchorDistance / 0.35)
        let anchorWeight = candidateCount >= 2 ? 0.30 : 0.05
        let appearanceWeight = candidateCount >= 2 ? 0.55 : 0.80
        return (appearanceWeight * appearance) + (0.15 * sizeScore) + (anchorWeight * initialAnchorScore)
    }

    func updateMatched(face: DetectedFace, t: Double, enterThreshold: Int, observedPeopleCount: Int, reidentified: Bool = false) {
        let previousState = state
        let elapsed = max(0.001, t - lastSeenTime)
        let observedVelocityX = (face.x - lastKnownX) / elapsed
        let observedVelocityY = (face.y - lastKnownY) / elapsed
        velocityX = 0.65 * velocityX + 0.35 * max(-1.0, min(1.0, observedVelocityX))
        velocityY = 0.65 * velocityY + 0.35 * max(-1.0, min(1.0, observedVelocityY))
        consecutiveHits += 1
        consecutiveMisses = 0
        totalDetections += 1
        lastSeenTime = t
        lastMatchConfidence = face.confidence
        if observedPeopleCount >= 2 {
            sceneAnchorX = 0.85 * sceneAnchorX + 0.15 * face.x
            sceneAnchorY = 0.85 * sceneAnchorY + 0.15 * face.y
        }

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
            if reidentified {
                state = .reappeared
            } else if consecutiveHits >= enterThreshold {
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
        if let faceLandmarks = face.landmarkEmbedding {
            if landmarkEmbedding == nil {
                landmarkEmbedding = faceLandmarks
            } else if let currentLandmarks = landmarkEmbedding {
                landmarkEmbedding = zip(currentLandmarks, faceLandmarks).map {
                    0.92 * $0.0 + 0.08 * $0.1
                }
            }
        }
        if let currentFeaturePrint = face.featurePrint {
            featurePrint = currentFeaturePrint
        }
        if previousState != .exited {
            rememberAppearance(from: face)
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
// Layout Confirmation State Machine (Issue #4)
// =============================================================================

func layoutType(for ids: [Int]) -> String {
    switch ids.count {
    case 1: return "single"
    case 2: return "split_two"
    case 3: return "split_three"
    default: return ids.count >= 3 ? "split_three" : (ids.count == 2 ? "split_two" : "single")
    }
}

/// Commits a layout (or person-id set) change only after the candidate has been
/// observed continuously for a confirmation window. A candidate that flaps away
/// before the window elapses never commits; a confirmed candidate back-dates its
/// segment start to its first observation, clamped so the previous segment keeps
/// its minimum duration.
struct LayoutConfirmationMachine {
    let confirmationSec: Double
    let minDurationSec: Double

    private(set) var currentLayout: String = "single"
    private(set) var currentPersonIds: [Int] = [1]
    private(set) var currentStart: Double = 0.0
    private(set) var candidateLayout: String? = nil
    private(set) var candidatePersonIds: [Int] = []
    private(set) var candidateStartTime: Double = 0.0
    private(set) var stableDuration: Double = 0.0
    private(set) var rawSegments: [LayoutSegment] = []

    mutating func initialize(ids: [Int], layout: String) {
        currentPersonIds = ids
        currentLayout = layout
        currentStart = 0.0
        candidateLayout = nil
        candidatePersonIds = []
        candidateStartTime = 0.0
        stableDuration = 0.0
    }

    mutating func observe(t: Double, targetIds: [Int]) {
        let targetLayout = layoutType(for: targetIds)

        if targetLayout == currentLayout && targetIds == currentPersonIds {
            candidateLayout = nil
            candidatePersonIds = []
            candidateStartTime = 0.0
            stableDuration = 0.0
            return
        }

        if candidateLayout != targetLayout || candidatePersonIds != targetIds {
            candidateLayout = targetLayout
            candidatePersonIds = targetIds
            candidateStartTime = t
            stableDuration = 0.0
        } else {
            stableDuration = t - candidateStartTime
        }

        let elapsedInLayout = t - currentStart
        guard stableDuration >= confirmationSec, elapsedInLayout >= minDurationSec else { return }

        let commitStart = max(candidateStartTime, currentStart + minDurationSec)
        rawSegments.append(LayoutSegment(
            start: currentStart,
            end: commitStart,
            number_of_people: currentPersonIds.count,
            layout_type: currentLayout,
            person_ids: currentPersonIds
        ))
        currentStart = commitStart
        currentLayout = candidateLayout!
        currentPersonIds = candidatePersonIds
        candidateLayout = nil
        candidatePersonIds = []
        candidateStartTime = 0.0
        stableDuration = 0.0
    }

    mutating func finalize(durationSec: Double) {
        rawSegments.append(LayoutSegment(
            start: currentStart,
            end: durationSec,
            number_of_people: currentPersonIds.count,
            layout_type: currentLayout,
            person_ids: currentPersonIds
        ))
    }
}

// =============================================================================
// MultiPersonReframingEngine
// =============================================================================

class MultiPersonReframingEngine {
    // Configuration thresholds
    let sampleRateFps: Double = 3.5            // 3.5 samples/sec (285ms intervals)
    let enterThresholdHits: Int = 3            // Require 3 consecutive hits (~0.85s) to confirm entry
    let confidenceThreshold: Double = 0.45     // Minimum face detection confidence
    let lostTimeoutSec: Double = 3.5           // 3.5s hysteresis for temporary occlusion / looking away
    let minLayoutDurationSec: Double = 3.0      // Hysteresis: minimum duration a layout must persist
    let layoutConfirmationSec: Double = 1.0     // A new layout must stay observed this long before switching
    let maxSupportedPeople: Int = 3            // Up to 3 active people in 9:16 vertical canvas

    func execute(videoPath: String, startSec: Double, durationSec: Double) -> String {
        let tempDir = NSTemporaryDirectory() + "clipon_reframe_\(UUID().uuidString)"
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

            let faceRequest = VNDetectFaceRectanglesRequest()
            let humanRequest = VNDetectHumanRectanglesRequest()
            let landmarkRequest = VNDetectFaceLandmarksRequest()
            let handler = VNImageRequestHandler(cgImage: cg, options: [:])
            try? handler.perform([faceRequest, humanRequest, landmarkRequest])

            var detected: [DetectedFace] = []
            let faceObservations = faceRequest.results ?? []
            let landmarkObservations = landmarkRequest.results ?? []
            let humanObservations = (humanRequest.results ?? []).filter {
                Double($0.confidence) >= confidenceThreshold
            }

            for face in faceObservations {
                if Double(face.confidence) < confidenceThreshold { continue }
                let containingHuman = humanObservations.first {
                    containsCenter($0.boundingBox, face.boundingBox)
                }
                let appearanceBox = containingHuman.map {
                    upperBodyBoundingBox($0.boundingBox)
                } ?? face.boundingBox
                let landmark = landmarkObservations
                    .filter { boundingBoxOverlap($0.boundingBox, face.boundingBox) > 0.05 }
                    .max { boundingBoxOverlap($0.boundingBox, face.boundingBox) < boundingBoxOverlap($1.boundingBox, face.boundingBox) }
                let embedding = extractPixelEmbedding(from: cg, boundingBox: appearanceBox)
                let landmarkEmbedding = landmark.flatMap { extractLandmarkEmbedding(from: $0) }
                let featurePrint = makeFeaturePrint(from: cg, boundingBox: appearanceBox)
                detected.append(DetectedFace(
                    x: face.boundingBox.midX,
                    y: 1.0 - face.boundingBox.midY,
                    width: face.boundingBox.width,
                    height: face.boundingBox.height,
                    embedding: embedding,
                    landmarkEmbedding: landmarkEmbedding,
                    featurePrint: featurePrint,
                    confidence: Double(face.confidence)
                ))
                allFaceCenters.append(face.boundingBox.midX)
            }

            if humanObservations.count >= 3 {
                for human in humanObservations {
                let humanBox = human.boundingBox
                let alreadyRepresented = faceObservations.contains {
                    containsCenter(humanBox, $0.boundingBox)
                }
                if alreadyRepresented { continue }

                let upperBodyBox = upperBodyBoundingBox(humanBox)
                let embedding = extractPixelEmbedding(from: cg, boundingBox: upperBodyBox)
                let featurePrint = makeFeaturePrint(from: cg, boundingBox: upperBodyBox)
                detected.append(DetectedFace(
                    x: upperBodyBox.midX,
                    y: 1.0 - upperBodyBox.midY,
                    width: upperBodyBox.width,
                    height: upperBodyBox.height,
                    embedding: embedding,
                    landmarkEmbedding: nil,
                    featurePrint: featurePrint,
                    confidence: Double(human.confidence) * 0.75
                ))
                allFaceCenters.append(upperBodyBox.midX)
                }
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
            var matchedTrackIndices = Set<Int>()

            // A. Match existing tracks with appearance, position, motion, size, confidence, and history.
            for track in personTracks {
                if track.state == .exited {
                    track.updateMissed(t: t, lostTimeout: lostTimeoutSec)
                    continue
                }
                var bestFaceIdx = -1
                var bestScore = 0.0
                let matchThreshold = track.state == .temporarilyLost ? 0.42 : (track.state == .exited ? 0.62 : 0.52)

                for (fIdx, face) in sample.faces.enumerated() {
                    if assignedFaceIndices.contains(fIdx) { continue }
                    let score = track.associationScore(face: face, t: t, candidateCount: sample.faces.count)
                    if score > bestScore {
                        bestScore = score
                        bestFaceIdx = fIdx
                    }
                }

                if bestFaceIdx != -1 && bestScore >= matchThreshold {
                    assignedFaceIndices.insert(bestFaceIdx)
                    let reidentified = track.state == .exited
                    if let trackIndex = personTracks.firstIndex(where: { $0.id == track.id }) {
                        matchedTrackIndices.insert(trackIndex)
                    }
                    track.updateMatched(
                        face: sample.faces[bestFaceIdx],
                        t: t,
                        enterThreshold: enterThresholdHits,
                        observedPeopleCount: sample.faces.count,
                        reidentified: reidentified
                    )
                } else {
                    track.updateMissed(t: t, lostTimeout: lostTimeoutSec)
                }
            }

            // Re-identify lost/exited tracks before allowing any unmatched face to create a new ID.
            var reidentificationCandidates: [(score: Double, trackIndex: Int, faceIndex: Int)] = []
            for (trackIndex, track) in personTracks.enumerated() {
                if matchedTrackIndices.contains(trackIndex) || track.state == .visible || track.totalDetections < enterThresholdHits {
                    continue
                }
                for (faceIndex, face) in sample.faces.enumerated() where !assignedFaceIndices.contains(faceIndex) {
                    if track.state == .exited && face.confidence < 0.55 { continue }
                    let score = track.reidentificationScore(for: face, candidateCount: sample.faces.count)
                    if score >= 0.58 {
                        reidentificationCandidates.append((score, trackIndex, faceIndex))
                    }
                }
            }
            reidentificationCandidates.sort { $0.score > $1.score }
            for candidate in reidentificationCandidates {
                if matchedTrackIndices.contains(candidate.trackIndex) || assignedFaceIndices.contains(candidate.faceIndex) {
                    continue
                }
                let track = personTracks[candidate.trackIndex]
                matchedTrackIndices.insert(candidate.trackIndex)
                assignedFaceIndices.insert(candidate.faceIndex)
                track.updateMatched(
                    face: sample.faces[candidate.faceIndex],
                    t: t,
                    enterThreshold: enterThresholdHits,
                        observedPeopleCount: sample.faces.count,
                        reidentified: true
                )
            }

            // B. Unmatched faces discover new persistent individuals (up to maxSupportedPeople)
            if sample.faces.count > assignedFaceIndices.count && personTracks.count < maxSupportedPeople {
                for (fIdx, face) in sample.faces.enumerated() {
                    if !assignedFaceIndices.contains(fIdx) && personTracks.count < maxSupportedPeople {
                        let historicalMatch = personTracks.contains {
                            $0.totalDetections >= enterThresholdHits && $0.reidentificationScore(for: face, candidateCount: sample.faces.count) >= 0.58
                        }
                        if historicalMatch { continue }
                        let newId = personTracks.count + 1
                        let newTrack = PersonTrackInstance(
                            id: newId,
                            name: "Person \(newId)",
                            x: face.x,
                            y: face.y,
                            w: face.width,
                            h: face.height,
                            emb: face.embedding,
                            landmarkEmb: face.landmarkEmbedding,
                            featurePrint: face.featurePrint,
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

        // Step 3: Layout State Machine — candidate confirmation + minimum duration hysteresis
        var rawSegments: [LayoutSegment] = []

        if !frameRecords.isEmpty {
            var machine = LayoutConfirmationMachine(
                confirmationSec: layoutConfirmationSec,
                minDurationSec: minLayoutDurationSec
            )

            // Initialize first frame layout
            let firstIds = frameRecords[0].activePersonIds.filter { id in confirmedTracks.contains(where: { $0.id == id }) }
            if !firstIds.isEmpty {
                machine.initialize(ids: firstIds, layout: layoutType(for: firstIds))
            } else if let firstTrack = confirmedTracks.first {
                machine.initialize(ids: [firstTrack.id], layout: "single")
            }

            for rec in frameRecords {
                let validIds = rec.activePersonIds.filter { id in confirmedTracks.contains(where: { $0.id == id }) }
                let targetIds = validIds.isEmpty ? machine.currentPersonIds : validIds
                machine.observe(t: rec.t, targetIds: targetIds)
            }

            machine.finalize(durationSec: durationSec)
            rawSegments = machine.rawSegments
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

        let p1Kfs: [TrackingKeyframeOutput] = (confirmedTracks.first?.keyframes ?? []).map {
            TrackingKeyframeOutput(t: $0.t, x: $0.x, y: $0.y)
        }
        let p2Kfs: [TrackingKeyframeOutput] = (confirmedTracks.count > 1 ? confirmedTracks[1].keyframes : []).map {
            TrackingKeyframeOutput(t: $0.t, x: $0.x, y: $0.y)
        }

        let p1Json = (try? String(data: encoder.encode(p1Kfs), encoding: .utf8)) ?? "[]"
        let p2Json = (try? String(data: encoder.encode(p2Kfs), encoding: .utf8)) ?? "[]"

        let avgCenterX = allFaceCenters.isEmpty ? 0.5 : (allFaceCenters.reduce(0, +) / Double(allFaceCenters.count))
        let p1HomeX = confirmedTracks.first?.lastKnownX ?? 0.26
        let p1HomeY = confirmedTracks.first?.lastKnownY ?? 0.38
        let p2HomeX = confirmedTracks.count > 1 ? confirmedTracks[1].lastKnownX : 0.78
        let p2HomeY = confirmedTracks.count > 1 ? confirmedTracks[1].lastKnownY : 0.38
        let twoFacesDetected = confirmedTracks.count >= 2

        let payloadJson = String(
            format: "{\"top_center_x\": %.3f, \"top_center_y\": %.3f, \"bottom_center_x\": %.3f, \"bottom_center_y\": %.3f, \"two_faces_detected\": %@, \"person_1_keyframes\": %@, \"person_2_keyframes\": %@, \"people\": %@, \"segments\": %@}",
            p1HomeX, p1HomeY,
            p2HomeX, p2HomeY,
            twoFacesDetected ? "true" : "false",
            p1Json,
            p2Json,
            peopleJson,
            segmentsJson
        )

        return String(
            format: "{\"avg_center_x\": %.3f, \"face_detected\": %@, \"width\": 1920, \"height\": 1080, \"tracking\": %@, \"podcast\": %@}",
            avgCenterX,
            allFaceCenters.isEmpty ? "false" : "true",
            payloadJson,
            payloadJson
        )
    }
}

// =============================================================================
// Layout Confirmation Self-Test (Issue #4 acceptance sequences)
// =============================================================================

func runLayoutSelfTest() -> Int {
    let dt = 1.0 / 3.5
    var failures = 0

    func run(_ observations: [(Double, [Int])], durationSec: Double) -> [LayoutSegment] {
        var machine = LayoutConfirmationMachine(confirmationSec: 1.0, minDurationSec: 3.0)
        machine.initialize(ids: observations[0].1, layout: layoutType(for: observations[0].1))
        for (t, ids) in observations {
            machine.observe(t: t, targetIds: ids)
        }
        machine.finalize(durationSec: durationSec)
        return machine.rawSegments
    }

    func expect(_ name: String, _ observations: [(Double, [Int])], durationSec: Double,
                _ expected: [(String, [Int], Double, Double)]) {
        let segments = run(observations, durationSec: durationSec)
        let ok = segments.count == expected.count && zip(segments, expected).allSatisfy { seg, exp in
            seg.layout_type == exp.0 && seg.person_ids == exp.1
                && abs(seg.start - exp.2) < 0.01 && abs(seg.end - exp.3) < 0.01
        }
        if ok {
            print("PASS \(name)")
        } else {
            failures += 1
            print("FAIL \(name)")
            for seg in segments {
                print("      got \(seg.start)-\(seg.end) \(seg.layout_type) \(seg.person_ids)")
            }
        }
    }

    let two: [Int] = [1, 2]
    let three: [Int] = [1, 2, 3]
    let p13: [Int] = [1, 3]

    // Roadmap example: brief flaps must never commit. 2,2,3,2,3,2 → stay at 2.
    expect("flap suppression",
           [(0.0, two), (1 * dt, two), (2 * dt, three), (3 * dt, two), (4 * dt, three), (5 * dt, two)],
           durationSec: 6.0,
           [("split_two", two, 0.0, 6.0)])

    // Roadmap example: stable candidate switches. 3,3,3,3,3 after >3s of 2 → switch to 3.
    var holdTwo: [(Double, [Int])] = (0..<15).map { (Double($0) * dt, two) } // ~4.0s of split_two
    let switchStart = 15 * dt
    holdTwo.append(contentsOf: (0..<6).map { (switchStart + Double($0) * dt, three) })
    expect("stable switch to three",
           holdTwo,
           durationSec: switchStart + 6 * dt,
           [("split_two", two, 0.0, switchStart), ("split_three", three, switchStart, switchStart + 6 * dt)])

    // A single differing observation after 3s in the current layout must not flip it.
    var blip: [(Double, [Int])] = (0..<14).map { (Double($0) * dt, two) }
    blip.append((14 * dt, p13))
    blip.append(contentsOf: (15..<20).map { (Double($0) * dt, two) })
    expect("single-blip after 3s ignored",
           blip,
           durationSec: 20 * dt,
           [("split_two", two, 0.0, 20 * dt)])

    // Person-id change inside the same layout type needs the same confirmation.
    var idSwap: [(Double, [Int])] = (0..<14).map { (Double($0) * dt, two) }
    idSwap.append(contentsOf: (0..<5).map { (14 * dt + Double($0) * dt, p13) })
    expect("confirmed id swap within split_two",
           idSwap,
           durationSec: 19 * dt,
           [("split_two", two, 0.0, 14 * dt), ("split_two", p13, 14 * dt, 19 * dt)])

    // A candidate interrupted mid-window restarts its confirmation from scratch.
    var interrupted: [(Double, [Int])] = (0..<14).map { (Double($0) * dt, two) }
    interrupted.append(contentsOf: (0..<2).map { (14 * dt + Double($0) * dt, three) }) // 0.57s < 1s
    interrupted.append(contentsOf: (0..<2).map { (16 * dt + Double($0) * dt, two) })
    interrupted.append(contentsOf: (0..<5).map { (18 * dt + Double($0) * dt, three) })
    expect("interrupted candidate restarts",
           interrupted,
           durationSec: 23 * dt,
           [("split_two", two, 0.0, 18 * dt), ("split_three", three, 18 * dt, 23 * dt)])

    // Early switch is clamped so the previous segment keeps its minimum duration.
    var early: [(Double, [Int])] = (0..<11).map { (Double($0) * dt, two) } // 3.14s >= min duration
    let earlyCandidate = 11 * dt
    early.append((earlyCandidate, three))
    early.append((earlyCandidate + dt, two)) // flaps back before confirming
    expect("clamped early commit stays two",
           early,
           durationSec: 13 * dt,
           [("split_two", two, 0.0, 13 * dt)])

    // Candidate observed before min duration elapsed: commit is clamped to
    // currentStart + minDuration so the first segment never shrinks below 3s.
    var clamp: [(Double, [Int])] = [(0.0, two)]
    clamp.append(contentsOf: (1...14).map { (Double($0) * dt, three) })
    expect("clamped commit start",
           clamp,
           durationSec: 15 * dt,
           [("split_two", two, 0.0, 3.0), ("split_three", three, 3.0, 15 * dt)])

    print(failures == 0 ? "selftest-layout: ALL PASS" : "selftest-layout: \(failures) FAILURE(S)")
    return failures
}

// Entry Point
let args = CommandLine.arguments
if args.count == 2 && args[1] == "--selftest-layout" {
    exit(Int32(runLayoutSelfTest()))
}
if args.count < 4 {
    print("{\"avg_center_x\": 0.5, \"face_detected\": false, \"tracking\": {\"top_center_x\": 0.26, \"top_center_y\": 0.38, \"bottom_center_x\": 0.78, \"bottom_center_y\": 0.38, \"two_faces_detected\": false, \"people\": [], \"segments\": []}, \"podcast\": {\"top_center_x\": 0.26, \"top_center_y\": 0.38, \"bottom_center_x\": 0.78, \"bottom_center_y\": 0.38, \"two_faces_detected\": false, \"people\": [], \"segments\": []}}")
    exit(0)
}

let videoPath = args[1]
let startSec = Double(args[2]) ?? 0.0
let durationSec = Double(args[3]) ?? 1.0

let engine = MultiPersonReframingEngine()
let output = engine.execute(videoPath: videoPath, startSec: startSec, durationSec: durationSec)
print(output)