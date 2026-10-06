import Foundation
import Vision
import AppKit

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

struct PointSample {
    var x: Double
    var y: Double
}

func runTracker() {
    let args = CommandLine.arguments
    guard args.count >= 4 else {
        print("{\"avg_center_x\": 0.5, \"face_detected\": false, \"width\": 1920, \"height\": 1080, \"podcast\": {\"person_a_x\": 0.28, \"person_a_y\": 0.38, \"person_b_x\": 0.72, \"person_b_y\": 0.38, \"two_persons_detected\": false}}")
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

    var allFaceCenters: [Double] = []
    var samplesA: [PointSample] = []
    var samplesB: [PointSample] = []

    for filePath in frameFiles {
        guard let img = NSImage(contentsOfFile: filePath),
              let cg = img.cgImage(forProposedRect: nil, context: nil, hints: nil) else { continue }

        let req = VNDetectFaceRectanglesRequest()
        let handler = VNImageRequestHandler(cgImage: cg, options: [:])
        try? handler.perform([req])

        guard let results = req.results, !results.isEmpty else { continue }

        var frameFaces: [PointSample] = []
        for face in results {
            let box = face.boundingBox
            let midX = Double(box.midX)
            // Vision has origin at bottom-left, convert to standard top-left video coordinates
            let midY = 1.0 - Double(box.midY)
            allFaceCenters.append(midX)
            frameFaces.append(PointSample(x: midX, y: midY))
        }

        // Sort faces left-to-right
        frameFaces.sort { $0.x < $1.x }

        if frameFaces.count >= 2 {
            // First is Left Person (Person A), Last is Right Person (Person B)
            samplesA.append(frameFaces.first!)
            samplesB.append(frameFaces.last!)
        } else if let single = frameFaces.first {
            // Single face in frame: classify based on side of table
            if single.x < 0.50 {
                samplesA.append(single)
            } else {
                samplesB.append(single)
            }
        }
    }

    let avgCenterX = allFaceCenters.isEmpty ? 0.5 : (allFaceCenters.reduce(0, +) / Double(allFaceCenters.count))
    let detected = !allFaceCenters.isEmpty

    // Compute Person A (Left Speaker) center
    let rawAX = samplesA.isEmpty ? 0.28 : (samplesA.map { $0.x }.reduce(0, +) / Double(samplesA.count))
    let rawAY = samplesA.isEmpty ? 0.38 : (samplesA.map { $0.y }.reduce(0, +) / Double(samplesA.count))
    let personAX = min(max(0.15, rawAX), 0.48)
    let personAY = min(max(0.20, rawAY), 0.60)

    // Compute Person B (Right Speaker) center
    let rawBX = samplesB.isEmpty ? 0.72 : (samplesB.map { $0.x }.reduce(0, +) / Double(samplesB.count))
    let rawBY = samplesB.isEmpty ? 0.38 : (samplesB.map { $0.y }.reduce(0, +) / Double(samplesB.count))
    let personBX = min(max(0.52, rawBX), 0.85)
    let personBY = min(max(0.20, rawBY), 0.60)

    let twoPersonsDetected = !samplesA.isEmpty && !samplesB.isEmpty

    print(String(
        format: "{\"avg_center_x\": %.3f, \"face_detected\": %@, \"width\": 1920, \"height\": 1080, \"podcast\": {\"person_a_x\": %.3f, \"person_a_y\": %.3f, \"person_b_x\": %.3f, \"person_b_y\": %.3f, \"two_persons_detected\": %@}}",
        avgCenterX,
        detected ? "true" : "false",
        personAX, personAY,
        personBX, personBY,
        twoPersonsDetected ? "true" : "false"
    ))
}

runTracker()
