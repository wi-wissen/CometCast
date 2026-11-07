// SPDX-License-Identifier: GPL-3.0-or-later
// Receiver video on iOS: H.264 access units from the Rust core (pass-through mode) are decoded
// by VideoToolbox (hardware) and shown as a Flutter texture. Same channel protocol as the
// Android bridge (lib/core/android_video.dart; Kotlin: video/VideoBridge.kt):
//   MethodChannel 'de.cometcast/video'         start -> {textureId} / queue {data, ptsUs, key} / stop
//   EventChannel  'de.cometcast/video/events'  size / needKeyFrame / error
// There is no software decoder on iOS (docs/ios.md): when this path fails the receiver says so.
//
// Rules: nothing waits for the decoder. At most `maxPending` units wait; a unit that does not
// fit is dropped and a key frame is requested. Decoding starts at a key frame with parameter
// sets; a change of SPS/PPS (the sender rotated or resized) builds a new decoder session.
//
// UNCERTAIN: never compiled or run on a device.

import CoreMedia
import Flutter
import UIKit
import VideoToolbox

final class VideoBridge: NSObject, FlutterTexture, FlutterStreamHandler {
    private let textures: FlutterTextureRegistry
    private var textureId: Int64 = -1
    private var sink: FlutterEventSink?

    private let queue = DispatchQueue(label: "de.cometcast.video.decode", qos: .userInteractive)
    private let lock = NSLock()
    private var latest: CVPixelBuffer?
    private var pending = 0
    private static let maxPending = 4

    private var session: VTDecompressionSession?
    private var format: CMVideoFormatDescription?
    private var sps: [UInt8] = []
    private var pps: [UInt8] = []
    private var waitingForKey = true
    private var size = CGSize.zero

    init(messenger: FlutterBinaryMessenger, textures: FlutterTextureRegistry) {
        self.textures = textures
        super.init()
        FlutterMethodChannel(name: "de.cometcast/video", binaryMessenger: messenger)
            .setMethodCallHandler { [weak self] call, result in self?.handle(call, result: result) }
        FlutterEventChannel(name: "de.cometcast/video/events", binaryMessenger: messenger)
            .setStreamHandler(self)
    }

    // MARK: channel

    private func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        switch call.method {
        case "start":
            if textureId < 0 { textureId = textures.register(self) }
            queue.async { self.resetDecoder() }
            result(["textureId": textureId])
        case "queue":
            guard let a = call.arguments as? [String: Any],
                  let data = a["data"] as? FlutterStandardTypedData else { return result(false) }
            let key = a["key"] as? Bool ?? false
            let pts = (a["ptsUs"] as? NSNumber)?.int64Value ?? 0
            result(enqueue([UInt8](data.data), key: key, ptsUs: pts))
        case "stop":
            queue.async {
                self.invalidateSession()
                self.lock.lock(); self.latest = nil; self.lock.unlock()
            }
            if textureId >= 0 { textures.unregisterTexture(textureId); textureId = -1 }
            result(nil)
        default:
            result(FlutterMethodNotImplemented)
        }
    }

    func onListen(withArguments arguments: Any?, eventSink events: @escaping FlutterEventSink) -> FlutterError? {
        sink = events; return nil
    }
    func onCancel(withArguments arguments: Any?) -> FlutterError? { sink = nil; return nil }

    private func send(_ map: [String: Any]) { DispatchQueue.main.async { self.sink?(map) } }

    // MARK: FlutterTexture

    func copyPixelBuffer() -> Unmanaged<CVPixelBuffer>? {
        lock.lock(); defer { lock.unlock() }
        guard let latest else { return nil }
        return Unmanaged.passRetained(latest)
    }

    // MARK: decoding

    /// Returns false if the unit was dropped because the decoder is behind.
    private func enqueue(_ data: [UInt8], key: Bool, ptsUs: Int64) -> Bool {
        lock.lock()
        if pending >= VideoBridge.maxPending {
            lock.unlock()
            queue.async { self.waitingForKey = true }
            send(["type": "needKeyFrame"])
            return false
        }
        pending += 1
        lock.unlock()
        queue.async {
            self.decode(data, ptsUs: ptsUs)
            self.lock.lock(); self.pending -= 1; self.lock.unlock()
        }
        return true
    }

    private func decode(_ data: [UInt8], ptsUs: Int64) {
        guard let unit = H264Parsing.split(annexB: data) else { return }
        if let s = unit.sps, let p = unit.pps, s != sps || p != pps {
            sps = s; pps = p
            guard makeSession() else { return }
        }
        if waitingForKey {
            guard unit.isKey, session != nil else { return }
            waitingForKey = false
        }
        guard let session, let format else { return }

        var block: CMBlockBuffer?
        let length = unit.avcc.count
        guard CMBlockBufferCreateWithMemoryBlock(
            allocator: nil, memoryBlock: nil, blockLength: length, blockAllocator: nil,
            customBlockSource: nil, offsetToData: 0, dataLength: length, flags: 0,
            blockBufferOut: &block) == noErr, let block,
            unit.avcc.withUnsafeBytes({ CMBlockBufferReplaceDataBytes(
                with: $0.baseAddress!, blockBuffer: block, offsetIntoDestination: 0, dataLength: length) }) == noErr
        else { return }
        var sample: CMSampleBuffer?
        var timing = CMSampleTimingInfo(
            duration: .invalid, presentationTimeStamp: CMTime(value: ptsUs, timescale: 1_000_000),
            decodeTimeStamp: .invalid)
        var sampleSize = length
        guard CMSampleBufferCreateReady(
            allocator: nil, dataBuffer: block, formatDescription: format, sampleCount: 1,
            sampleTimingEntryCount: 1, sampleTimingArray: &timing, sampleSizeEntryCount: 1,
            sampleSizeArray: &sampleSize, sampleBufferOut: &sample) == noErr, let sample
        else { return }
        let status = VTDecompressionSessionDecodeFrame(
            session, sampleBuffer: sample, flags: [._EnableAsynchronousDecompression],
            infoFlagsOut: nil
        ) { [weak self] status, _, image, _, _ in
            guard let self, status == noErr, let image else {
                self?.queue.async { self?.waitingForKey = true }
                self?.send(["type": "needKeyFrame"])
                return
            }
            self.lock.lock(); self.latest = image; self.lock.unlock()
            self.textures.textureFrameAvailable(self.textureId)
        }
        if status == kVTInvalidSessionErr || status == kVTVideoDecoderMalfunctionErr {
            // Typical after the app was in the background: build a new session at the next key frame.
            invalidateSession()
            waitingForKey = true
            send(["type": "needKeyFrame"])
        } else if status != noErr {
            send(["type": "error", "message": "VTDecompressionSessionDecodeFrame failed (\(status))"])
        }
    }

    private func makeSession() -> Bool {
        invalidateSession()
        var fmt: CMVideoFormatDescription?
        let status = sps.withUnsafeBufferPointer { s in
            pps.withUnsafeBufferPointer { p in
                let pointers = [s.baseAddress!, p.baseAddress!]
                let sizes = [s.count, p.count]
                return CMVideoFormatDescriptionCreateFromH264ParameterSets(
                    allocator: nil, parameterSetCount: 2, parameterSetPointers: pointers,
                    parameterSetSizes: sizes, nalUnitHeaderLength: 4, formatDescriptionOut: &fmt)
            }
        }
        guard status == noErr, let fmt else {
            send(["type": "error", "message": "invalid SPS/PPS (\(status))"])
            return false
        }
        let attributes: [CFString: Any] = [
            kCVPixelBufferPixelFormatTypeKey: kCVPixelFormatType_32BGRA,
            kCVPixelBufferIOSurfacePropertiesKey: [String: Any](),
            kCVPixelBufferMetalCompatibilityKey: true,
        ]
        var created: VTDecompressionSession?
        let s = VTDecompressionSessionCreate(
            allocator: nil, formatDescription: fmt, decoderSpecification: nil,
            imageBufferAttributes: attributes as CFDictionary, outputCallback: nil,
            decompressionSessionOut: &created)
        guard s == noErr, let created else {
            send(["type": "error", "message": "VTDecompressionSessionCreate failed (\(s))"])
            return false
        }
        session = created
        format = fmt
        let dims = CMVideoFormatDescriptionGetDimensions(fmt)
        let newSize = CGSize(width: Int(dims.width), height: Int(dims.height))
        if newSize != size {
            size = newSize
            send(["type": "size", "width": Int(dims.width), "height": Int(dims.height)])
        }
        return true
    }

    private func invalidateSession() {
        if let s = session { VTDecompressionSessionInvalidate(s) }
        session = nil
        format = nil
    }

    private func resetDecoder() {
        invalidateSession()
        sps = []; pps = []
        waitingForKey = true
        size = .zero
    }
}
