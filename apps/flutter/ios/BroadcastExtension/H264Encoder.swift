// SPDX-License-Identifier: GPL-3.0-or-later
// Hardware H.264 encoding with VideoToolbox, tuned for a live screen: real time, low latency,
// no B-frames (output order = input order, the RTP packetiser needs no reordering), Constrained
// Baseline (what the WebRTC offer announces: profile-level-id=42e01f), a key frame every two
// seconds and on request, and Annex B output for `cc_ios_push_frame`.
//
// Threading: `encode` is called from one serial queue (SampleHandler's encode queue); the output
// callback runs on a VideoToolbox thread. Nothing here blocks the caller: when too many frames
// are in flight `encode` returns `.busy` and the caller drops the picture.
//
// UNCERTAIN: never compiled or run on a device. The property keys marked "best effort" are set
// without checking the result because not every iOS version knows them.

import CoreMedia
import Foundation
import VideoToolbox

protocol H264EncoderDelegate: AnyObject {
    /// SPS/PPS (Annex B) of a new encoder session; call `cc_ios_set_config` with it.
    func encoder(_ encoder: H264Encoder, didProduceParameterSets annexB: [UInt8])
    /// One access unit in Annex B. Key frames start with SPS and PPS.
    func encoder(_ encoder: H264Encoder, didProduce annexB: [UInt8], isKey: Bool, ptsMicroseconds: Int64)
    func encoder(_ encoder: H264Encoder, didFail message: String)
}

enum EncodeResult { case submitted, busy, failed }

final class H264Encoder {
    struct Settings: Equatable {
        var width: Int
        var height: Int
        var fps: Int
        var bitrate: Int
    }

    /// Frames handed to VideoToolbox whose output has not arrived yet. More than this and
    /// `encode` refuses (backpressure). With real-time encoding and no reordering 2 is plenty.
    static let maxInFlight = 2

    weak var delegate: H264EncoderDelegate?

    private var session: VTCompressionSession?
    private(set) var settings: Settings?
    private let lock = NSLock()
    private var inFlight = 0
    private var forceKey = true  // the first frame of a session is a key frame anyway
    private var scratch = [UInt8]()
    private(set) var framesEncoded = 0
    private(set) var framesDroppedByEncoder = 0

    deinit { invalidate() }

    /// (Re)create the session for `settings`. A different size needs a new session; the old
    /// one is flushed and released.
    func configure(_ settings: Settings) -> Bool {
        invalidate()
        var spec: [CFString: Any] = [
            kVTVideoEncoderSpecification_EnableHardwareAcceleratedVideoEncoder: true,
        ]
        // Low-latency rate control (iOS 14.5+). If the combination is refused, retry without.
        spec[kVTVideoEncoderSpecification_EnableLowLatencyRateControl] = true

        var created: VTCompressionSession?
        var status = create(settings, spec: spec, into: &created)
        if status != noErr {
            spec.removeValue(forKey: kVTVideoEncoderSpecification_EnableLowLatencyRateControl)
            status = create(settings, spec: spec, into: &created)
        }
        guard status == noErr, let s = created else {
            delegate?.encoder(self, didFail: "VTCompressionSessionCreate failed (\(status))")
            return false
        }
        func set(_ key: CFString, _ value: CFTypeRef, required: Bool = false) -> Bool {
            let r = VTSessionSetProperty(s, key: key, value: value)
            return r == noErr || !required
        }
        _ = set(kVTCompressionPropertyKey_RealTime, kCFBooleanTrue)
        _ = set(kVTCompressionPropertyKey_ProfileLevel, kVTProfileLevel_H264_Baseline_AutoLevel)
        _ = set(kVTCompressionPropertyKey_AllowFrameReordering, kCFBooleanFalse)
        _ = set(kVTCompressionPropertyKey_H264EntropyMode, kVTH264EntropyMode_CAVLC)
        _ = set(kVTCompressionPropertyKey_ExpectedFrameRate, settings.fps as CFNumber)
        _ = set(kVTCompressionPropertyKey_MaxKeyFrameIntervalDuration, 2 as CFNumber)
        _ = set(kVTCompressionPropertyKey_AverageBitRate, settings.bitrate as CFNumber)
        // At most 1.5x the average over any second (bytes, seconds).
        let limit = [Int(Double(settings.bitrate) * 1.5 / 8), 1] as CFArray
        _ = set(kVTCompressionPropertyKey_DataRateLimits, limit)
        // Best effort: speed over quality (macOS 13 / iOS 16 key; ignored if unknown).
        _ = set(kVTCompressionPropertyKey_PrioritizeEncodingSpeedOverQuality, kCFBooleanTrue)
        if VTCompressionSessionPrepareToEncodeFrames(s) != noErr {
            delegate?.encoder(self, didFail: "VTCompressionSessionPrepareToEncodeFrames failed")
            VTCompressionSessionInvalidate(s)
            return false
        }
        lock.lock()
        session = s
        self.settings = settings
        inFlight = 0
        forceKey = true
        lock.unlock()
        return true
    }

    private func create(_ s: Settings, spec: [CFString: Any], into out: inout VTCompressionSession?) -> OSStatus {
        VTCompressionSessionCreate(
            allocator: nil, width: Int32(s.width), height: Int32(s.height),
            codecType: kCMVideoCodecType_H264,
            encoderSpecification: spec as CFDictionary,
            imageBufferAttributes: nil, compressedDataAllocator: nil,
            outputCallback: nil, refcon: nil,  // per-frame handler below
            compressionSessionOut: &out)
    }

    /// The next frame will be a key frame (receiver lost a picture, new receiver, size change).
    func requestKeyFrame() {
        lock.lock(); forceKey = true; lock.unlock()
    }

    /// Submit one picture. Never blocks.
    func encode(_ pixelBuffer: CVPixelBuffer, pts: CMTime) -> EncodeResult {
        lock.lock()
        guard let session else { lock.unlock(); return .failed }
        if inFlight >= H264Encoder.maxInFlight { lock.unlock(); return .busy }
        let key = forceKey
        forceKey = false
        inFlight += 1
        lock.unlock()

        let props: CFDictionary? = key ? [kVTEncodeFrameOptionKey_ForceKeyFrame: true] as CFDictionary : nil
        let status = VTCompressionSessionEncodeFrame(
            session, imageBuffer: pixelBuffer, presentationTimeStamp: pts, duration: .invalid,
            frameProperties: props, infoFlagsOut: nil
        ) { [weak self] status, flags, sample in
            self?.handleOutput(status: status, flags: flags, sample: sample)
        }
        if status != noErr {
            lock.lock(); inFlight -= 1; if key { forceKey = true }; lock.unlock()
            return .failed
        }
        return .submitted
    }

    private func handleOutput(status: OSStatus, flags: VTEncodeInfoFlags, sample: CMSampleBuffer?) {
        lock.lock(); inFlight = max(0, inFlight - 1); lock.unlock()
        guard status == noErr, let sample, !flags.contains(.frameDropped),
              CMSampleBufferDataIsReady(sample) else {
            framesDroppedByEncoder += 1
            // A dropped frame breaks the chain of references: the next one must be a key frame.
            requestKeyFrame()
            return
        }
        var isKey = true
        if let attachments = CMSampleBufferGetSampleAttachmentsArray(sample, createIfNecessary: false)
            as? [[CFString: Any]], let first = attachments.first {
            isKey = !((first[kCMSampleAttachmentKey_NotSync] as? Bool) ?? false)
        }
        guard let format = CMSampleBufferGetFormatDescription(sample),
              let block = CMSampleBufferGetDataBuffer(sample) else { return }

        var lengthSize: Int32 = 4
        var out = [UInt8]()
        if isKey {
            var sets = [Data]()
            var count = 0
            CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
                format, parameterSetIndex: 0, parameterSetPointerOut: nil, parameterSetSizeOut: nil,
                parameterSetCountOut: &count, nalUnitHeaderLengthOut: &lengthSize)
            for i in 0..<count {
                var ptr: UnsafePointer<UInt8>?
                var size = 0
                if CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
                    format, parameterSetIndex: i, parameterSetPointerOut: &ptr, parameterSetSizeOut: &size,
                    parameterSetCountOut: nil, nalUnitHeaderLengthOut: nil) == noErr, let ptr {
                    sets.append(Data(bytes: ptr, count: size))
                }
            }
            let headers = AnnexB.annexB(parameterSets: sets)
            out.append(contentsOf: headers)
            delegate?.encoder(self, didProduceParameterSets: headers)
        } else {
            _ = CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
                format, parameterSetIndex: 0, parameterSetPointerOut: nil, parameterSetSizeOut: nil,
                parameterSetCountOut: nil, nalUnitHeaderLengthOut: &lengthSize)
        }

        // Copy the AVCC data (the block buffer may not be contiguous) and convert it.
        let length = CMBlockBufferGetDataLength(block)
        if scratch.count < length { scratch = [UInt8](repeating: 0, count: length) }
        let copied = scratch.withUnsafeMutableBytes { raw -> Bool in
            CMBlockBufferCopyDataBytes(block, atOffset: 0, dataLength: length, destination: raw.baseAddress!) == noErr
        }
        guard copied else { return }
        let units = scratch.withUnsafeBytes { raw in
            AnnexB.appendAnnexB(fromAVCC: UnsafeRawBufferPointer(rebasing: raw[0..<length]),
                                lengthSize: Int(lengthSize), into: &out)
        }
        guard units > 0 else {
            delegate?.encoder(self, didFail: "malformed AVCC output of the encoder")
            return
        }
        framesEncoded += 1
        let pts = CMSampleBufferGetPresentationTimeStamp(sample)
        let micros = pts.isValid ? Int64((CMTimeGetSeconds(pts) * 1_000_000).rounded()) : -1
        delegate?.encoder(self, didProduce: out, isKey: isKey, ptsMicroseconds: micros)
    }

    /// Flush and release the session.
    func invalidate() {
        lock.lock()
        let s = session
        session = nil
        settings = nil
        lock.unlock()
        if let s {
            VTCompressionSessionCompleteFrames(s, untilPresentationTimeStamp: .invalid)
            VTCompressionSessionInvalidate(s)
        }
    }
}
