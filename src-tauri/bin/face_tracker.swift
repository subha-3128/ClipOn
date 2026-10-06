import Foundation
import Vision
import AppKit

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

struct FaceDet {
    var x: Double
    var y: Double
    var yaw: Double
}

func runTracker() {
    let args = CommandLine.arguments
    guard args.count >= 4 else {
        print("{\"avg_center_x\": 0.5, \"face_detected\": false, \"podcast\": {\"top_center_x\": 0.28, \"top_center_y\": 0.38, \"bottom_center_x\": 0.72, \"bottom_center_y\": 0.38, \"two_faces_detected\": false}}")
        return
    }

    let videoPath = args[1]
    let startSec = Double(args[2]) ?? 0.0
    let durationSec = Double(args[3]) ?? 1.0

    let tempDir = NSTemporaryDirectory() + "clipon_trk_\(UUID().uuidString)"
    defer {
        try? FileManager.default.removeItem(atPath: tempDir)
    }

    let frameCount = 18
    let frameFiles = extractFrames(videoPath: videoPath, startSec: startSec, durationSec: durationSec, count: frameCount, outDir: tempDir)

    var allFaceCenters: [Double] = []
    var wideA_x: [Double] = []
    var wideA_y: [Double] = []
    var wideB_x: [Double] = []
    var wideB_y: [Double] = []

    var soloA_x: [Double] = []
    var soloA_y: [Double] = []
    var soloB_x: [Double] = []
    var soloB_y: [Double] = []

    var reactionTimeA: Double = startSec
    var reactionTimeB: Double = startSec

    var frameLabels: [String] = [] // "both", "a", "b"

    for (idx, filePath) in frameFiles.enumerated() {
        let t = startSec + (Double(idx) * (durationSec / Double(max(1, frameFiles.count))))
        guard let img = NSImage(contentsOfFile: filePath),
              let cg = img.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
            frameLabels.append("both")
            continue
        }

        let req = VNDetectFaceLandmarksRequest()
        let handler = VNImageRequestHandler(cgImage: cg, options: [:])
        try? handler.perform([req])

        var faces: [FaceDet] = []
        for face in req.results ?? [] {
            let x = face.boundingBox.midX
            let y = 1.0 - face.boundingBox.midY
            let yaw = face.yaw?.doubleValue ?? 0.0
            faces.append(FaceDet(x: x, y: y, yaw: yaw))
            allFaceCenters.append(x)
        }
        faces.sort { $0.x < $1.x }

        if faces.count >= 2 {
            // Wide shot with both speakers
            let fa = faces.first!
            let fb = faces.last!
            wideA_x.append(fa.x)
            wideA_y.append(fa.y)
            wideB_x.append(fb.x)
            wideB_y.append(fb.y)
            frameLabels.append("both")
        } else if let single = faces.first {
            // Solo shot: distinguish Person 1 (Host looking right, yaw > 0.1) vs Person 2 (Guest looking left, yaw < -0.1)
            if single.yaw > 0.08 || single.x < 0.42 {
                soloA_x.append(single.x)
                soloA_y.append(single.y)
                reactionTimeA = t
                frameLabels.append("a")
            } else {
                soloB_x.append(single.x)
                soloB_y.append(single.y)
                reactionTimeB = t
                frameLabels.append("b")
            }
        } else {
            frameLabels.append("both")
        }
    }

    // Build timeline shots
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

    // Reference Wide Positions: Person 1 (Left Table), Person 2 (Right Table)
    let refLeftX = wideA_x.isEmpty ? 0.26 : (wideA_x.reduce(0, +) / Double(wideA_x.count))
    let refLeftY = wideA_y.isEmpty ? 0.38 : (wideA_y.reduce(0, +) / Double(wideA_y.count))
    let refRightX = wideB_x.isEmpty ? 0.78 : (wideB_x.reduce(0, +) / Double(wideB_x.count))
    let refRightY = wideB_y.isEmpty ? 0.38 : (wideB_y.reduce(0, +) / Double(wideB_y.count))

    // Solo Positions
    let soloAX = soloA_x.isEmpty ? refLeftX : (soloA_x.reduce(0, +) / Double(soloA_x.count))
    let soloAY = soloA_y.isEmpty ? refLeftY : (soloA_y.reduce(0, +) / Double(soloA_y.count))
    let soloBX = soloB_x.isEmpty ? refRightX : (soloB_x.reduce(0, +) / Double(soloB_x.count))
    let soloBY = soloB_y.isEmpty ? refRightY : (soloB_y.reduce(0, +) / Double(soloB_y.count))

    let isMulticam = (!soloA_x.isEmpty || !soloB_x.isEmpty)
    let twoFacesDetected = (!wideA_x.isEmpty && !wideB_x.isEmpty) || (!soloA_x.isEmpty && !soloB_x.isEmpty)

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
