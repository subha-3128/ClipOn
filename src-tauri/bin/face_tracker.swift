import Foundation
import AVFoundation
import Vision

func detectFaceCenter(videoPath: String, startSec: Double, durationSec: Double) {
    let url = URL(fileURLWithPath: videoPath)
    let asset = AVURLAsset(url: url)
    guard let track = asset.tracks(withMediaType: .video).first else {
        print("{\"avg_center_x\": 0.5, \"face_detected\": false}")
        return
    }

    let size = track.naturalSize.applying(track.preferredTransform)
    let videoWidth = abs(size.width)
    let videoHeight = abs(size.height)

    let generator = AVAssetImageGenerator(asset: asset)
    generator.appliesPreferredTrackTransform = true
    generator.requestedTimeToleranceBefore = CMTime(seconds: 0.25, preferredTimescale: 600)
    generator.requestedTimeToleranceAfter = CMTime(seconds: 0.25, preferredTimescale: 600)

    let sampleCount = 6
    let step = max(0.5, durationSec / Double(sampleCount))
    var centers: [Double] = []

    for i in 0..<sampleCount {
        let t = startSec + Double(i) * step
        let cmTime = CMTime(seconds: t, preferredTimescale: 600)
        do {
            let cgImage = try generator.copyCGImage(at: cmTime, actualTime: nil)
            let request = VNDetectFaceRectanglesRequest()
            let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
            try handler.perform([request])
            if let results = request.results, !results.isEmpty {
                var maxArea: CGFloat = 0
                var bestX: Double = 0.5
                for face in results {
                    let area = face.boundingBox.width * face.boundingBox.height
                    if area > maxArea {
                        maxArea = area
                        bestX = Double(face.boundingBox.midX)
                    }
                }
                centers.append(bestX)
            }
        } catch {
        }
    }

    if centers.isEmpty {
        print(String(format: "{\"avg_center_x\": 0.5, \"face_detected\": false, \"width\": %.0f, \"height\": %.0f}", videoWidth, videoHeight))
    } else {
        let avg = centers.reduce(0.0, +) / Double(centers.count)
        // Clamp so the crop frame stays safely on screen
        let clamped = min(max(avg, 0.2), 0.8)
        print(String(format: "{\"avg_center_x\": %.3f, \"face_detected\": true, \"width\": %.0f, \"height\": %.0f}", clamped, videoWidth, videoHeight))
    }
}

let args = CommandLine.arguments
if args.count >= 4 {
    detectFaceCenter(
        videoPath: args[1],
        startSec: Double(args[2]) ?? 0.0,
        durationSec: Double(args[3]) ?? 1.0
    )
} else {
    print("{\"avg_center_x\": 0.5, \"face_detected\": false}")
}
