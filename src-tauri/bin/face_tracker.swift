
import Foundation
import Vision
import AppKit

struct FaceDet {
    var midX: Double
    var midY: Double
    var width: Double
    var height: Double
    var yaw: Double?
}

struct FrameData {
    var index: Int
    var timeSec: Double
    var faces: [FaceDet]
}

struct ShotSegment {
    var start: Double
    var end: Double
    var type: String // "both", "a", "b"
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

func runTracker() {
    let args = CommandLine.arguments
    guard args.count >= 4 else {
        print("{\"avg_center_x\": 0.5, \"face_detected\": false, \"podcast\": {\"top_center_x\": 0.28, \"top_center_y\": 0.36, \"bottom_center_x\": 0.72, \"bottom_center_y\": 0.36, \"two_faces_detected\": false}}")
        return
    }

    let videoPath = args[1]
    let startSec = Double(args[2]) ?? 0.0
    let durationSec = Double(args[3]) ?? 1.0

    let tempDir = NSTemporaryDirectory() + "clipon_trk_\(UUID().uuidString)"
    defer {
        try? FileManager.default.removeItem(atPath: tempDir)
    }

    let frameCount = 10
    let frameFiles = extractFrames(videoPath: videoPath, startSec: startSec, durationSec: durationSec, count: frameCount, outDir: tempDir)

    var frames: [FrameData] = []
    var allFaceCenters: [Double] = []

    for (idx, filePath) in frameFiles.enumerated() {
        guard let img = NSImage(contentsOfFile: filePath),
              let cg = img.cgImage(forProposedRect: nil, context: nil, hints: nil) else { continue }

        let req = VNDetectFaceLandmarksRequest()
        let handler = VNImageRequestHandler(cgImage: cg, options: [:])
        try? handler.perform([req])

        let timeOffset = Double(idx) * (durationSec / Double(max(1, frameFiles.count)))
        var dets: [FaceDet] = []

        for face in (req.results ?? []) {
            let w = face.boundingBox.width
            let h = face.boundingBox.height
            if w >= 0.04 && h >= 0.04 {
                let midX = Double(face.boundingBox.midX)
                let midY = Double(1.0 - face.boundingBox.midY)
                dets.append(FaceDet(
                    midX: midX,
                    midY: midY,
                    width: Double(w),
                    height: Double(h),
                    yaw: face.yaw?.doubleValue
                ))
                allFaceCenters.append(midX)
            }
        }
        frames.append(FrameData(index: idx, timeSec: startSec + timeOffset, faces: dets))
    }

    // 1. Identify wide 2-face frames
    var twoFaceFrames: [(FaceDet, FaceDet, Double)] = [] // (left, right, timeSec)
    for f in frames {
        if f.faces.count >= 2 {
            let sorted = f.faces.sorted { $0.midX < $1.midX }
            let left = sorted.first!
            let right = sorted.last!
            if (right.midX - left.midX) > 0.16 {
                twoFaceFrames.append((left, right, f.timeSec))
            }
        }
    }

    var twoFacesDetected = !twoFaceFrames.isEmpty
    var refLeftX = 0.28
    var refLeftY = 0.38
    var refRightX = 0.72
    var refRightY = 0.38
    var reactionTimeA = startSec + (durationSec * 0.25)
    var reactionTimeB = startSec + (durationSec * 0.50)

    if twoFacesDetected {
        refLeftX = twoFaceFrames.map { $0.0.midX }.reduce(0, +) / Double(twoFaceFrames.count)
        refLeftY = twoFaceFrames.map { $0.0.midY }.reduce(0, +) / Double(twoFaceFrames.count)
        refRightX = twoFaceFrames.map { $0.1.midX }.reduce(0, +) / Double(twoFaceFrames.count)
        refRightY = twoFaceFrames.map { $0.1.midY }.reduce(0, +) / Double(twoFaceFrames.count)
        reactionTimeA = twoFaceFrames.first!.2
        reactionTimeB = twoFaceFrames.first!.2
    } else {
        // Fallback: search wider video for a 2-shot reference frame
        let widerTempDir = NSTemporaryDirectory() + "clipon_wider_\(UUID().uuidString)"
        defer { try? FileManager.default.removeItem(atPath: widerTempDir) }
        let widerFrames = extractFrames(videoPath: videoPath, startSec: max(0.0, startSec - 120.0), durationSec: 240.0, count: 6, outDir: widerTempDir)
        for wPath in widerFrames {
            guard let wImg = NSImage(contentsOfFile: wPath),
                  let wCg = wImg.cgImage(forProposedRect: nil, context: nil, hints: nil) else { continue }
            let wReq = VNDetectFaceRectanglesRequest()
            let wHandler = VNImageRequestHandler(cgImage: wCg, options: [:])
            try? wHandler.perform([wReq])
            let valid = (wReq.results ?? []).filter { $0.boundingBox.width >= 0.04 }
            if valid.count >= 2 {
                let sorted = valid.sorted { $0.boundingBox.midX < $1.boundingBox.midX }
                let l = sorted.first!
                let r = sorted.last!
                if (r.boundingBox.midX - l.boundingBox.midX) > 0.16 {
                    twoFacesDetected = true
                    refLeftX = Double(l.boundingBox.midX)
                    refLeftY = Double(1.0 - l.boundingBox.midY)
                    refRightX = Double(r.boundingBox.midX)
                    refRightY = Double(1.0 - r.boundingBox.midY)
                    break
                }
            }
        }
    }

    // Classify each frame into "both", "a", or "b"
    var soloASamples: [(Double, Double, Double)] = [] // (x, y, timeSec)
    var soloBSamples: [(Double, Double, Double)] = [] // (x, y, timeSec)
    var frameLabels: [String] = []

    for f in frames {
        if f.faces.count >= 2 {
            let sorted = f.faces.sorted { $0.midX < $1.midX }
            if (sorted.last!.midX - sorted.first!.midX) > 0.16 {
                frameLabels.append("both")
                continue
            }
        }
        if let only = f.faces.first {
            let distA = abs(only.midX - refLeftX)
            let distB = abs(only.midX - refRightX)
            if distA < distB || (only.yaw ?? 0) > 0.25 {
                soloASamples.append((only.midX, only.midY, f.timeSec))
                frameLabels.append("a")
            } else {
                soloBSamples.append((only.midX, only.midY, f.timeSec))
                frameLabels.append("b")
            }
        } else {
            frameLabels.append("both")
        }
    }

    if let bestB = soloBSamples.first {
        reactionTimeB = bestB.2
    }
    if let bestA = soloASamples.first {
        reactionTimeA = bestA.2
    }

    let isMulticam = frameLabels.contains("a") || frameLabels.contains("b")

    // Timeline shots
    var shots: [ShotSegment] = []
    if !frameLabels.isEmpty {
        var currType = frameLabels[0]
        var currStart = 0.0
        let step = durationSec / Double(frameLabels.count)
        for i in 1..<frameLabels.count {
            if frameLabels[i] != currType {
                let end = Double(i) * step
                shots.append(ShotSegment(start: currStart, end: end, type: currType))
                currType = frameLabels[i]
                currStart = end
            }
        }
        shots.append(ShotSegment(start: currStart, end: durationSec, type: currType))
    }

    var shotsJsonParts: [String] = []
    for s in shots {
        shotsJsonParts.append(String(format: "{\"start\": %.2f, \"end\": %.2f, \"type\": \"%@\"}", s.start, s.end, s.type))
    }
    let shotsJson = "[" + shotsJsonParts.joined(separator: ", ") + "]"

    let avgCenterX = allFaceCenters.isEmpty ? 0.5 : (allFaceCenters.reduce(0, +) / Double(allFaceCenters.count))

    let soloAX = soloASamples.isEmpty ? refLeftX : (soloASamples.map { $0.0 }.reduce(0, +) / Double(soloASamples.count))
    let soloAY = soloASamples.isEmpty ? refLeftY : (soloASamples.map { $0.1 }.reduce(0, +) / Double(soloASamples.count))
    let soloBX = soloBSamples.isEmpty ? refRightX : (soloBSamples.map { $0.0 }.reduce(0, +) / Double(soloBSamples.count))
    let soloBY = soloBSamples.isEmpty ? refRightY : (soloBSamples.map { $0.1 }.reduce(0, +) / Double(soloBSamples.count))

    print(String(
        format: "{\"avg_center_x\": %.3f, \"face_detected\": %@, \"width\": 1920, \"height\": 1080, \"podcast\": {\"top_center_x\": %.3f, \"top_center_y\": %.3f, \"bottom_center_x\": %.3f, \"bottom_center_y\": %.3f, \"solo_a_x\": %.3f, \"solo_a_y\": %.3f, \"solo_b_x\": %.3f, \"solo_b_y\": %.3f, \"top_reaction_t\": %.2f, \"bottom_reaction_t\": %.2f, \"is_multicam\": %@, \"two_faces_detected\": %@, \"shots\": %@}}",
        avgCenterX,
        allFaceCenters.isEmpty ? "false" : "true",
        refLeftX, refLeftY,
        refRightX, refRightY,
        soloAX, soloAY,
        soloBX, soloBY,
        reactionTimeA,
        reactionTimeB,
        isMulticam ? "true" : "false",
        twoFacesDetected ? "true" : "false",
        shotsJson
    ))
}

runTracker()
