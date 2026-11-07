// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.capture

import java.util.concurrent.atomic.AtomicInteger

/**
 * Tags the output of every encoder of a session with a generation number. When the capture size
 * changes the service swaps the encoder: only the active generation may deliver config and frames,
 * so output of the old encoder that is still in flight (drain thread, main looper queue) can never
 * mix into the stream of the new one. No Android types here, so it is unit tested on the JVM.
 */
class GenerationGate {
    private val counter = AtomicInteger(0)

    @Volatile
    private var active = 0

    /** A new generation number; it delivers nothing until [activate] is called with it. */
    fun allocate(): Int = counter.incrementAndGet()

    fun activate(generation: Int) {
        active = generation
    }

    fun accepts(generation: Int): Boolean = generation == active

    /** Forget everything (a new session starts). */
    fun reset() {
        counter.set(0)
        active = 0
    }
}

/**
 * Holds the codec config of an encoder back until the first key frame is about to be sent, so
 * that the new size is published together with the first picture of the new encoder and not
 * before it (many encoders emit their SPS/PPS right at start).
 */
class ConfigHold<T> {
    private var held: T? = null

    fun hold(config: T) {
        held = config
    }

    /** The held config, once, to be sent in front of the first key frame. */
    fun takeForKeyFrame(): T? {
        val c = held
        held = null
        return c
    }
}
