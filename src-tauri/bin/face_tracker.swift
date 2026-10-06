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

func runTracker() {
    let args = CommandLine.arguments
    guard args.count >= 4 else {
        print("{\"avg_center_x\": 0.5, \"face_detected\": false, \"width\": 1920, \"height\": 1080}")
        return
    }

    let videoPath = args[1]
    let startSec = Double(args[2]) ?? 0.0
    let durationSec = Double(args[3]) ?? 1.0

    let tempDir = NSTemporaryDirectory() + "clipon_trk_\(UUID().uuidString)"
    defer {
        try? FileManager.default.removeItem(atPath: tempDir)
    }

    let frameCount = 8
    let frameFiles = extractFrames(videoPath: videoPath, startSec: startSec, durationSec: durationSec, count: frameCount, outDir: tempDir)

    var allFaceCenters: [Double] = []

    for filePath in frameFiles {
        guard let img = NSImage(contentsOfFile: filePath),
              let cg = img.cgImage(forProposedRect: nil, context: nil, hints: nil) else { continue }

        let req = VNDetectFaceRectanglesRequest()
        let handler = VNImageRequestHandler(cgImage: cg, options: [:])
        try? handler.perform([req])

        if let results = req.results, !results.isEmpty {
            for face in results {
                let box = face.boundingBox
                allFaceCenters.append(Double(box.midX))
            }
        }
    }

    let avgCenterX = allFaceCenters.isEmpty ? 0.5 : (allFaceCenters.reduce(0, +) / Double(allFaceCenters.count))
    let detected = !allFaceCenters.isEmpty

    print(String(
        format: "{\"avg_center_x\": %.3f, \"face_detected\": %@, \"width\": 1920, \"height\": 1080}",
        avgCenterX,
        detected ? "true" : "false"
    ))
}

runTracker()
