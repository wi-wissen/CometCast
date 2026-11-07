// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.video

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class H264UtilTest {
    /** Writes bits MSB first, with Exp-Golomb helpers: builds an SPS without an encoder. */
    private class BitWriter {
        private val bits = ArrayList<Int>()

        fun u(
            value: Int,
            n: Int,
        ) {
            for (i in n - 1 downTo 0) bits.add((value shr i) and 1)
        }

        fun ue(value: Int) {
            val v = value + 1
            val len = 32 - Integer.numberOfLeadingZeros(v)
            repeat(len - 1) { bits.add(0) }
            u(v, len)
        }

        fun toBytes(): ByteArray {
            bits.add(1) // rbsp_stop_one_bit
            while (bits.size % 8 != 0) bits.add(0)
            val out = ByteArray(bits.size / 8)
            for (i in out.indices) {
                var b = 0
                for (j in 0 until 8) b = (b shl 1) or bits[i * 8 + j]
                out[i] = b.toByte()
            }
            return out
        }
    }

    /** SPS NAL (header byte 0x67 included) of a Baseline picture of the given size, optional crop. */
    private fun sps(
        widthMbs: Int,
        heightMbs: Int,
        cropBottom: Int = 0,
    ): ByteArray {
        val w = BitWriter()
        w.u(66, 8) // profile_idc: baseline
        w.u(0xc0, 8)
        w.u(31, 8) // level 3.1
        w.ue(0) // sps id
        w.ue(0) // log2_max_frame_num_minus4
        w.ue(2) // pic_order_cnt_type 2
        w.ue(1) // max_num_ref_frames
        w.u(0, 1)
        w.ue(widthMbs - 1)
        w.ue(heightMbs - 1)
        w.u(1, 1) // frame_mbs_only_flag
        w.u(1, 1) // direct_8x8_inference_flag
        if (cropBottom > 0) {
            w.u(1, 1)
            w.ue(0)
            w.ue(0)
            w.ue(0)
            w.ue(cropBottom)
        } else {
            w.u(0, 1)
        }
        w.u(0, 1) // vui_parameters_present_flag
        return byteArrayOf(0x67) + w.toBytes()
    }

    private val startCode4 = byteArrayOf(0, 0, 0, 1)
    private val startCode3 = byteArrayOf(0, 0, 1)

    @Test
    fun spsSizeOf720p() {
        // 1280 x 720 is coded as 80 x 46 macroblocks (736 rows); the bottom crop is 8 crop units,
        // each 2 luma rows for 4:2:0 frame coding, so 736 - 16 = 720.
        assertEquals(Pair(1280, 720), H264Util.spsSize(sps(80, 46, cropBottom = 8)))
    }

    @Test
    fun spsSizeWithoutCrop() {
        assertEquals(Pair(640, 480), H264Util.spsSize(sps(40, 30)))
    }

    /** Inserts 0x03 after every 00 00 that is followed by 00..03, as an encoder does. */
    private fun escape(raw: ByteArray): ByteArray {
        val out = ArrayList<Byte>()
        var zeros = 0
        for (b in raw) {
            if (zeros >= 2 && (b.toInt() and 0xff) <= 3) {
                out.add(3)
                zeros = 0
            }
            out.add(b)
            zeros = if (b.toInt() == 0) zeros + 1 else 0
        }
        return out.toByteArray()
    }

    /** A general SPS: every field the parser has to skip or evaluate can be chosen. */
    private fun spsFull(
        profile: Int = 66,
        constraints: Int = 0xc0,
        level: Int = 31,
        spsId: Int = 0,
        chromaFormat: Int = 1,
        separatePlanes: Boolean = false,
        scalingLists: Boolean = false,
        pocType: Int = 2,
        widthMbs: Int = 40,
        heightMapUnits: Int = 30,
        frameMbsOnly: Boolean = true,
        crop: List<Int>? = null,
        vui: Boolean = false,
    ): ByteArray {
        val w = BitWriter()
        w.u(profile, 8)
        w.u(constraints, 8)
        w.u(level, 8)
        w.ue(spsId)
        if (profile in listOf(100, 110, 122, 244, 44, 83, 86, 118, 128)) {
            w.ue(chromaFormat)
            if (chromaFormat == 3) w.u(if (separatePlanes) 1 else 0, 1)
            w.ue(0)
            w.ue(0)
            w.u(0, 1)
            w.u(if (scalingLists) 1 else 0, 1)
            if (scalingLists) {
                val n = if (chromaFormat != 3) 8 else 12
                for (i in 0 until n) {
                    // List i present only for even i: first flat (delta 0 ends it early), then a
                    // list with explicit deltas.
                    val present = i % 2 == 0
                    w.u(if (present) 1 else 0, 1)
                    if (!present) continue
                    val size = if (i < 6) 16 else 64
                    if (i % 4 == 0) {
                        w.ue(0) // se(-8) would end it; se(0)=ue(0): nextScale 8, runs on
                        repeat(size - 1) { w.ue(0) }
                    } else {
                        // delta -8 -> nextScale 0: the rest of the list repeats the last value.
                        w.ue(16) // se(-8) is code number 16
                    }
                }
            }
        }
        w.ue(0) // log2_max_frame_num_minus4
        w.ue(pocType)
        when (pocType) {
            0 -> w.ue(4) // log2_max_pic_order_cnt_lsb_minus4
            1 -> {
                w.u(0, 1) // delta_pic_order_always_zero_flag
                w.ue(3) // offset_for_non_ref_pic: se(-2)
                w.ue(2) // offset_for_top_to_bottom_field: se(+1)
                w.ue(3) // num_ref_frames_in_pic_order_cnt_cycle
                repeat(3) { w.ue(1) } // offset_for_ref_frame[i]: se(+1)
            }
        }
        w.ue(1) // max_num_ref_frames
        w.u(0, 1) // gaps
        w.ue(widthMbs - 1)
        w.ue(heightMapUnits - 1)
        w.u(if (frameMbsOnly) 1 else 0, 1)
        if (!frameMbsOnly) w.u(0, 1) // mb_adaptive_frame_field_flag
        w.u(1, 1) // direct_8x8_inference_flag
        if (crop != null) {
            w.u(1, 1)
            crop.forEach { w.ue(it) }
        } else {
            w.u(0, 1)
        }
        w.u(if (vui) 1 else 0, 1)
        return byteArrayOf(0x67) + w.toBytes()
    }

    @Test
    fun spsSizeSurvivesEmulationPrevention() {
        // profile 66, constraint 0, level 0 give the bytes 00 00, and sps id 100 makes the next
        // byte 0x03: the encoder has to insert an emulation prevention byte (00 00 03 03).
        val raw = spsFull(constraints = 0, level = 0, spsId = 100)
        assertTrue(
            "the test SPS must contain 00 00 03",
            (1 until raw.size - 2).any { raw[it].toInt() == 0 && raw[it + 1].toInt() == 0 && raw[it + 2].toInt() == 3 },
        )
        val escaped = escape(raw)
        assertEquals(raw.size + 1, escaped.size)
        assertTrue(escaped.indices.any { it >= 3 && escaped[it - 3].toInt() == 0 && escaped[it - 2].toInt() == 0 && escaped[it - 1].toInt() == 3 })
        // Without removing the inserted 0x03 everything after it would be shifted by 8 bits.
        assertEquals(Pair(640, 480), H264Util.spsSize(escaped))
        assertEquals(Pair(640, 480), H264Util.spsSize(raw))
    }

    @Test
    fun highProfileWithScalingLists() {
        // The bottom crop of 4 units is 8 rows in 4:2:0 and 4 rows in 4:4:4.
        for ((chroma, height) in listOf(1 to 1080, 3 to 1084)) {
            assertEquals(
                "chroma $chroma",
                Pair(1920, height),
                H264Util.spsSize(
                    spsFull(
                        profile = 100,
                        chromaFormat = chroma,
                        scalingLists = true,
                        widthMbs = 120,
                        heightMapUnits = 68,
                        crop = listOf(0, 0, 0, 4),
                    ),
                ),
            )
        }
    }

    @Test
    fun highProfileWithoutScalingLists() {
        assertEquals(Pair(1280, 720), H264Util.spsSize(spsFull(profile = 100, widthMbs = 80, heightMapUnits = 45)))
        assertEquals(Pair(640, 480), H264Util.spsSize(spsFull(profile = 244, chromaFormat = 3)))
    }

    @Test
    fun separateColourPlanesAreCroppedInSamples() {
        // ChromaArrayType 0: the crop unit is one sample, not two.
        assertEquals(Pair(638, 478), H264Util.spsSize(spsFull(profile = 244, chromaFormat = 3, separatePlanes = true, crop = listOf(0, 2, 0, 2))))
        // Without separate planes 4:4:4 also crops in samples.
        assertEquals(Pair(638, 478), H264Util.spsSize(spsFull(profile = 244, chromaFormat = 3, crop = listOf(0, 2, 0, 2))))
        // 4:2:0 crops in units of two.
        assertEquals(Pair(636, 476), H264Util.spsSize(spsFull(profile = 100, crop = listOf(0, 2, 0, 2))))
    }

    @Test
    fun pictureOrderCountTypes() {
        assertEquals(Pair(640, 480), H264Util.spsSize(spsFull(pocType = 0)))
        assertEquals(Pair(640, 480), H264Util.spsSize(spsFull(pocType = 1)))
        assertEquals(Pair(640, 480), H264Util.spsSize(spsFull(profile = 100, pocType = 1, crop = listOf(0, 0, 0, 0))))
    }

    @Test
    fun frameCroppingOnAllSides() {
        // 4:2:0 frame coding, units of 2 samples: (8 + 3) * 2 = 22 columns, (2 + 3) * 2 = 10 rows off.
        assertEquals(Pair(640 - 22, 480 - 10), H264Util.spsSize(spsFull(crop = listOf(8, 3, 2, 3))))
    }

    @Test
    fun interlacedSpsDoublesTheMapUnits() {
        // frame_mbs_only_flag = 0: the height is 2 map units per row of macroblock pairs, and the
        // vertical crop unit doubles (4 rows for 4:2:0).
        assertEquals(Pair(640, 960), H264Util.spsSize(spsFull(frameMbsOnly = false)))
        assertEquals(Pair(640, 960 - 4 * 2), H264Util.spsSize(spsFull(frameMbsOnly = false, crop = listOf(0, 0, 0, 2))))
    }

    @Test
    fun spsWithVuiStillParses() {
        assertEquals(Pair(640, 480), H264Util.spsSize(spsFull(vui = true)))
    }

    @Test
    fun unescapeRemovesOnlyEmulationPreventionBytes() {
        val out = H264Util.unescape(byteArrayOf(0x67, 0, 0, 3, 1, 0, 3, 0, 0, 3, 0), 1)
        assertArrayEquals(byteArrayOf(0, 0, 1, 0, 3, 0, 0, 0), out)
    }

    @Test
    fun garbageSpsGivesNull() {
        assertNull(H264Util.spsSize(byteArrayOf(0x67)))
        assertNull(H264Util.spsSize(ByteArray(0)))
    }

    @Test
    fun splitsThreeAndFourByteStartCodes() {
        val sps = sps(40, 30)
        val pps = byteArrayOf(0x68, 0xce.toByte(), 0x3c, 0x80.toByte())
        val idr = byteArrayOf(0x65, 1, 2, 3)
        val data = startCode4 + sps + startCode3 + pps + startCode4 + idr
        val nals = H264Util.splitNals(data)
        assertEquals(listOf(7, 8, 5), nals.map { it.type })
        assertTrue(H264Util.hasIdr(nals))
        // The NAL bytes exclude the start code.
        assertArrayEquals(pps, data.copyOfRange(nals[1].start, nals[1].end))
        assertArrayEquals(idr, data.copyOfRange(nals[2].start, nals[2].end))
    }

    @Test
    fun parameterSetsKeepTheirStartCodes() {
        val sps = sps(40, 30)
        val pps = byteArrayOf(0x68, 1, 2)
        val data = startCode4 + sps + startCode4 + pps + startCode4 + byteArrayOf(0x65, 9)
        val sets = H264Util.parameterSets(data, H264Util.splitNals(data))
        assertNotNull(sets)
        assertArrayEquals(startCode4 + sps, sets!!.first)
        assertArrayEquals(startCode4 + pps, sets.second)
    }

    @Test
    fun deltaFrameHasNoIdrAndNoParameterSets() {
        val data = startCode4 + byteArrayOf(0x41, 5, 5)
        val nals = H264Util.splitNals(data)
        assertFalse(H264Util.hasIdr(nals))
        assertNull(H264Util.parameterSets(data, nals))
    }

    @Test
    fun garbageHasNoNals() {
        assertTrue(H264Util.splitNals(byteArrayOf(1, 2, 3, 4)).isEmpty())
        assertTrue(H264Util.splitNals(ByteArray(0)).isEmpty())
    }
}
