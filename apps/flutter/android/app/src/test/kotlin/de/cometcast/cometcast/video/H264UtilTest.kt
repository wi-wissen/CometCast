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

    @Test
    fun spsSizeSurvivesEmulationPrevention() {
        // 0x00 0x00 0x03 inside the payload must be removed before parsing.
        val raw = sps(40, 30)
        val escaped = ArrayList<Byte>()
        var zeros = 0
        for (b in raw) {
            if (zeros >= 2 && (b.toInt() and 0xff) <= 3) {
                escaped.add(3)
                zeros = 0
            }
            escaped.add(b)
            zeros = if (b.toInt() == 0) zeros + 1 else 0
        }
        assertEquals(Pair(640, 480), H264Util.spsSize(escaped.toByteArray()))
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
