import Foundation
import AVFoundation
import Vision

struct Point {
    var x: Double
    var y: Double
}

func detectFaces(videoPath: String, startSec: Double, durationSec: Double) {
    let url = URL(fileURLWithPath: videoPath)
    let asset = AVURLAsset(url: url)
    guard let track = asset.tracks(withMediaType: .video).first else {
        print("{\"avg_center_x\": 0.5, \"face_detected\": false, \"podcast\": {\"top_center_x\": 0.28, \"top_center_y\": 0.36, \"bottom_center_x\": 0.72, \"bottom_center_y\": 0.36, \"two_faces_detected\": false}}")
        return
    }

    let size = track.naturalSize.applying(track.preferredTransform)
    let videoWidth = Double(abs(size.width))
    let videoHeight = Double(abs(size.height))

    let generator = AVAssetImageGenerator(asset: asset)
    generator.appliesPreferredTrackTransform = true
    generator.requestedTimeToleranceBefore = CMTime(seconds: 0.2, preferredTimescale: 600)
    generator.requestedTimeToleranceAfter = CMTime(seconds: 0.2, preferredTimescale: 600)

    let sampleCount = 8
    let step = max(0.25, durationSec / Double(sampleCount))
    var singleCenters: [Double] = []
    var leftSamples: [Point] = []
    var rightSamples: [Point] = []
    var observedTwoFacesInAFrame = false

    for i in 0..<sampleCount {
        let t = startSec + Double(i) * step
        let cmTime = CMTime(seconds: t, preferredTimescale: 600)
        do {
            let cgImage = try generator.copyCGImage(at: cmTime, actualTime: nil)
            let request = VNDetectFaceRectanglesRequest()
            let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
            try handler.perform([request])

            if let results = request.results, !results.isEmpty {
                // Filter out tiny false positives
                let validFaces = results.filter { $0.boundingBox.width >= 0.04 && $0.boundingBox.height >= 0.04 }
                let faces = validFaces.isEmpty ? results : validFaces

                // For single face tracker
                var maxArea: CGFloat = 0
                var bestX: Double = 0.5
                for face in faces {
                    let area = face.boundingBox.width * face.boundingBox.height
                    if area > maxArea {
                        maxArea = area
                        bestX = Double(face.boundingBox.midX)
                    }
                }
                singleCenters.append(bestX)

                // For podcast two-face detection
                if faces.count >= 2 {
                    let sorted = faces.sorted { $0.boundingBox.midX < $1.boundingBox.midX }
                    let left = sorted.first!
                    let right = sorted.last!
                    let diffX = Double(right.boundingBox.midX - left.boundingBox.midX)
                    if diffX > 0.15 {
                        observedTwoFacesInAFrame = true
                        leftSamples.append(Point(x: Double(left.boundingBox.midX), y: Double(1.0 - left.boundingBox.midY)))
                        rightSamples.append(Point(x: Double(right.boundingBox.midX), y: Double(1.0 - right.boundingBox.midY)))
                    } else {
                        let mid = Double(left.boundingBox.midX)
                        let midY = Double(1.0 - left.boundingBox.midY)
                        if mid < 0.5 {
                            leftSamples.append(Point(x: mid, y: midY))
                        } else {
                            rightSamples.append(Point(x: mid, y: midY))
                        }
                    }
                } else if let only = faces.first {
                    let mid = Double(only.boundingBox.midX)
                    let midY = Double(1.0 - only.boundingBox.midY)
                    if mid < 0.5 {
                        leftSamples.append(Point(x: mid, y: midY))
                    } else {
                        rightSamples.append(Point(x: mid, y: midY))
                    }
                }
            }
        } catch {}
    }

    let topX: Double
    let topY: Double
    if !leftSamples.isEmpty {
        topX = min(max(leftSamples.map { $0.x }.reduce(0, +) / Double(leftSamples.count), 0.10), 0.48)
        topY = min(max(leftSamples.map { $0.y }.reduce(0, +) / Double(leftSamples.count), 0.15), 0.70)
    } else {
        topX = 0.28
        topY = 0.36
    }

    let botX: Double
    let botY: Double
    if !rightSamples.isEmpty {
        botX = min(max(rightSamples.map { $0.x }.reduce(0, +) / Double(rightSamples.count), 0.52), 0.90)
        botY = min(max(rightSamples.map { $0.y }.reduce(0, +) / Double(rightSamples.count), 0.15), 0.70)
    } else {
        botX = 0.72
        botY = 0.36
    }

    let twoFacesDetected = observedTwoFacesInAFrame || (!leftSamples.isEmpty && !rightSamples.isEmpty)
    let singleAvg = singleCenters.isEmpty ? 0.5 : min(max(singleCenters.reduce(0, +) / Double(singleCenters.count), 0.2), 0.8)

    print(String(format: "{\"avg_center_x\": %.3f, \"face_detected\": %@, \"width\": %.0f, \"height\": %.0f, \"podcast\": {\"top_center_x\": %.3f, \"top_center_y\": %.3f, \"bottom_center_x\": %.3f, \"bottom_center_y\": %.3f, \"two_faces_detected\": %@}}",
                 singleAvg,
                 singleCenters.isEmpty ? "false" : "true",
                 videoWidth,
                 videoHeight,
                 topX, topY, botX, botY,
                 twoFacesDetected ? "true" : "false"))
}

let args = CommandLine.arguments
if args.count >= 4 {
    detectFaces(videoPath: args[1], startSec: Double(args[2]) ?? 0.0, durationSec: Double(args[3]) ?? 1.0)
} else {
    print("{\"avg_center_x\": 0.5, \"face_detected\": false, \"podcast\": {\"top_center_x\": 0.28, \"top_center_y\": 0.36, \"bottom_center_x\": 0.72, \"bottom_center_y\": 0.36, \"two_faces_detected\": false}}")
}
