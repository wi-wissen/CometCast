// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.video

/**
 * Small helpers for H.264 Annex B access units (start codes), without Android classes so they
 * are covered by plain JVM unit tests.
 */
object H264Util {
    const val NAL_IDR = 5
    const val NAL_SPS = 7
    const val NAL_PPS = 8

    /** One NAL unit inside an Annex B buffer: [start] is the first byte of the NAL header. */
    data class Nal(
        val start: Int,
        val end: Int,
        val type: Int,
        /** Offset of the start code that precedes the NAL (3 or 4 bytes before [start]). */
        val prefixStart: Int,
    )

    /** Splits an Annex B buffer (3 or 4 byte start codes) into NAL units. Garbage gives an empty list. */
    fun splitNals(data: ByteArray): List<Nal> {
        val out = ArrayList<Nal>()
        var i = 0
        var current = -1
        var currentPrefix = 0
        while (i + 2 < data.size) {
            if (data[i].toInt() == 0 && data[i + 1].toInt() == 0 && data[i + 2].toInt() == 1) {
                if (current >= 0) {
                    // The 4-byte form has one more zero in front of the 3-byte pattern.
                    var end = i
                    if (end > current && data[end - 1].toInt() == 0) end--
                    out.add(Nal(current, end, data[current].toInt() and 0x1f, currentPrefix))
                }
                currentPrefix = if (i > 0 && data[i - 1].toInt() == 0) i - 1 else i
                current = i + 3
                i += 3
            } else {
                i++
            }
        }
        if (current in 0 until data.size) {
            out.add(Nal(current, data.size, data[current].toInt() and 0x1f, currentPrefix))
        }
        return out
    }

    fun hasIdr(nals: List<Nal>): Boolean = nals.any { it.type == NAL_IDR }

    /** The SPS and PPS of a key frame as Annex B (with start codes), or null if one is missing. */
    fun parameterSets(
        data: ByteArray,
        nals: List<Nal>,
    ): Pair<ByteArray, ByteArray>? {
        val sps = nals.firstOrNull { it.type == NAL_SPS } ?: return null
        val pps = nals.firstOrNull { it.type == NAL_PPS } ?: return null
        return Pair(
            data.copyOfRange(sps.prefixStart, sps.end),
            data.copyOfRange(pps.prefixStart, pps.end),
        )
    }

    /**
     * Picture size from an SPS NAL (the bytes after the NAL header byte are parsed; [sps] may
     * contain the header and emulation prevention bytes). Returns width x height after cropping,
     * or null if the SPS cannot be parsed.
     */
    fun spsSize(sps: ByteArray): Pair<Int, Int>? {
        return try {
            val rbsp = unescape(sps, 1)
            val r = BitReader(rbsp)
            val profile = r.bits(8)
            r.bits(8) // constraint flags and reserved bits
            r.bits(8) // level_idc
            r.ue() // seq_parameter_set_id
            var chromaFormat = 1
            var chromaFormatIdc = 1
            if (profile in HIGH_PROFILES) {
                chromaFormatIdc = r.ue()
                chromaFormat = chromaFormatIdc
                // separate_colour_plane_flag = 1: ChromaArrayType is 0 (three monochrome planes).
                if (chromaFormat == 3 && r.bits(1) == 1) chromaFormat = 0
                r.ue() // bit_depth_luma_minus8
                r.ue() // bit_depth_chroma_minus8
                r.bits(1) // qpprime_y_zero_transform_bypass_flag
                if (r.bits(1) == 1) { // seq_scaling_matrix_present_flag
                    val lists = if (chromaFormatIdc != 3) 8 else 12
                    for (i in 0 until lists) {
                        if (r.bits(1) == 1) skipScalingList(r, if (i < 6) 16 else 64)
                    }
                }
            }
            r.ue() // log2_max_frame_num_minus4
            when (r.ue()) { // pic_order_cnt_type
                0 -> r.ue()
                1 -> {
                    r.bits(1)
                    r.se()
                    r.se()
                    val n = r.ue()
                    for (i in 0 until n) r.se()
                }
            }
            r.ue() // max_num_ref_frames
            r.bits(1) // gaps_in_frame_num_value_allowed_flag
            val widthMbs = r.ue() + 1
            val heightMapUnits = r.ue() + 1
            val frameMbsOnly = r.bits(1)
            if (frameMbsOnly == 0) r.bits(1) // mb_adaptive_frame_field_flag
            r.bits(1) // direct_8x8_inference_flag
            var cropL = 0
            var cropR = 0
            var cropT = 0
            var cropB = 0
            if (r.bits(1) == 1) {
                cropL = r.ue()
                cropR = r.ue()
                cropT = r.ue()
                cropB = r.ue()
            }
            val subW = if (chromaFormat == 1 || chromaFormat == 2) 2 else 1
            val subH = if (chromaFormat == 1) 2 else 1
            val cropUnitX = if (chromaFormat == 0) 1 else subW
            val cropUnitY = (if (chromaFormat == 0) 1 else subH) * (2 - frameMbsOnly)
            val w = widthMbs * 16 - cropUnitX * (cropL + cropR)
            val h = (2 - frameMbsOnly) * heightMapUnits * 16 - cropUnitY * (cropT + cropB)
            if (w <= 0 || h <= 0 || w > 16384 || h > 16384) null else Pair(w, h)
        } catch (e: IndexOutOfBoundsException) {
            null
        } catch (e: IllegalArgumentException) {
            null
        }
    }

    private val HIGH_PROFILES = setOf(100, 110, 122, 244, 44, 83, 86, 118, 128, 138, 139, 134, 135)

    private fun skipScalingList(
        r: BitReader,
        size: Int,
    ) {
        var last = 8
        var next = 8
        for (j in 0 until size) {
            if (next != 0) {
                val delta = r.se()
                next = (last + delta + 256) % 256
            }
            last = if (next == 0) last else next
        }
    }

    /** Removes emulation prevention bytes (00 00 03 -> 00 00), starting at [from]. */
    fun unescape(
        data: ByteArray,
        from: Int,
    ): ByteArray {
        val out = ByteArray(maxOf(data.size - from, 0))
        var n = 0
        var zeros = 0
        for (i in from until data.size) {
            val b = data[i].toInt() and 0xff
            if (zeros >= 2 && b == 3) {
                zeros = 0
                continue
            }
            out[n++] = data[i]
            zeros = if (b == 0) zeros + 1 else 0
        }
        return out.copyOf(n)
    }

    /** MSB-first bit reader with Exp-Golomb codes. Throws IndexOutOfBoundsException at the end. */
    class BitReader(private val data: ByteArray) {
        private var pos = 0

        fun bits(n: Int): Int {
            var v = 0
            repeat(n) {
                val byte = data[pos ushr 3].toInt() and 0xff
                v = (v shl 1) or ((byte shr (7 - (pos and 7))) and 1)
                pos++
            }
            return v
        }

        fun ue(): Int {
            var zeros = 0
            while (bits(1) == 0) {
                zeros++
                if (zeros > 31) throw IllegalArgumentException("bad Exp-Golomb code")
            }
            if (zeros == 0) return 0
            return ((1L shl zeros) - 1 + bits(zeros).toLong()).toInt()
        }

        fun se(): Int {
            val k = ue()
            return if (k and 1 == 1) (k + 1) / 2 else -(k / 2)
        }
    }
}
