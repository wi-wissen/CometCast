// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.video

/**
 * The decisions of [HardwareVideoDecoder] without any Android type, so that they run in plain JVM
 * unit tests: key frame gating, re-requesting key frames with a backoff, the "may the codec still
 * render" rule of the surface teardown, the oversize limit and the unwrapped timestamps.
 *
 * Thread safety: every method is synchronized; the state is used by the worker thread, the
 * platform thread (surface callbacks) and the Dart-facing `queue` calls.
 */
class DecoderState(
    private val nowMs: () -> Long = { System.nanoTime() / 1_000_000 },
    private val retryIntervalMs: Long = KEY_RETRY_INTERVAL_MS,
    private val maxKeyRequests: Int = MAX_KEY_REQUESTS,
    private val maxOversize: Int = MAX_OVERSIZE,
) {
    private var needKey = true
    private var surfaceLost = false
    private var tornDown = false
    private var keyRequests = 0
    private var lastKeyRequestMs = Long.MIN_VALUE
    private var oversizeInARow = 0

    enum class Overflow { NONE, NEED_KEY, GIVE_UP }

    /** Delta frames are dropped while this is true. */
    @Synchronized
    fun needsKeyFrame(): Boolean = needKey

    /** A unit was lost or the codec was reset: wait for the next key frame and ask for one. */
    @Synchronized
    fun requireKeyFrame() {
        if (!needKey) {
            keyRequests = 0
            lastKeyRequestMs = Long.MIN_VALUE
        }
        needKey = true
    }

    /**
     * Should the unit be handed to the codec? A delta frame is refused while a key frame is
     * awaited; a key frame ends the wait.
     */
    @Synchronized
    fun admit(isKey: Boolean): Boolean {
        if (!needKey) return true
        if (!isKey) return false
        needKey = false
        keyRequests = 0
        lastKeyRequestMs = Long.MIN_VALUE
        oversizeInARow = 0
        return true
    }

    /**
     * While a key frame is awaited: true when the sender must be asked again (at once for the
     * first time, then once per retry interval). After [maxKeyRequests] unanswered requests
     * [exhausted] turns true.
     */
    @Synchronized
    fun keyRequestDue(): Boolean {
        if (!needKey || surfaceLost || tornDown) return false
        if (keyRequests >= maxKeyRequests) return false
        val now = nowMs()
        if (lastKeyRequestMs != Long.MIN_VALUE && now - lastKeyRequestMs < retryIntervalMs) return false
        lastKeyRequestMs = now
        keyRequests++
        return true
    }

    /** No key frame came although it was asked for [maxKeyRequests] times. */
    @Synchronized
    fun exhausted(): Boolean = needKey && keyRequests >= maxKeyRequests &&
        lastKeyRequestMs != Long.MIN_VALUE && nowMs() - lastKeyRequestMs >= retryIntervalMs

    /**
     * A unit did not fit the codec's input buffer. The first few times a new key frame is
     * requested; if it stays too large the decoder gives up (the error makes the app switch to the
     * software decoder) instead of asking forever.
     */
    @Synchronized
    fun oversize(): Overflow {
        oversizeInARow++
        needKey = true
        return if (oversizeInARow >= maxOversize) Overflow.GIVE_UP else Overflow.NEED_KEY
    }

    @Synchronized
    fun surfaceDestroyed() {
        surfaceLost = true
        needKey = true
    }

    @Synchronized
    fun surfaceAvailable() {
        surfaceLost = false
        tornDown = false // a new surface may be rendered to again
        requireKeyFrame()
    }

    /** The decoder is being stopped or the surface is being torn down: nothing may render. */
    @Synchronized
    fun beginTeardown() {
        tornDown = true
    }

    @Synchronized
    fun reset() {
        tornDown = false
        surfaceLost = false
        needKey = true
        keyRequests = 0
        lastKeyRequestMs = Long.MIN_VALUE
        oversizeInARow = 0
    }

    @Synchronized
    fun isSurfaceLost(): Boolean = surfaceLost

    /** `releaseOutputBuffer(index, render)`: never true once a teardown began. */
    @Synchronized
    fun mayRender(): Boolean = !surfaceLost && !tornDown

    companion object {
        const val KEY_RETRY_INTERVAL_MS = 1_000L
        const val MAX_KEY_REQUESTS = 10
        const val MAX_OVERSIZE = 3
    }
}

/**
 * Presentation times arrive as 32 bit RTP clock values converted to microseconds, so they wrap
 * after [WRAP_US] (about 13 hours). This makes them a growing 64 bit value.
 */
class PtsUnwrapper(
    private val wrapUs: Long = WRAP_US,
) {
    private var last = Long.MIN_VALUE
    private var epochs = 0L

    @Synchronized
    fun unwrap(ptsUs: Long): Long {
        if (ptsUs < 0) return ptsUs // unknown stays unknown
        if (last != Long.MIN_VALUE && ptsUs < last - wrapUs / 2) epochs++
        last = ptsUs
        return ptsUs + epochs * wrapUs
    }

    companion object {
        /** 2^32 ticks of 90 kHz, in microseconds. */
        const val WRAP_US = 47_721_858_844L
    }
}

/**
 * Bounded FIFO between the Dart-facing `queue` call and the worker. When it is full the oldest
 * delta frame goes (a key frame only if nothing else is left); the caller then has to ask for a
 * key frame because the stream has a hole.
 */
class UnitQueue<T>(
    private val capacity: Int,
    private val isKey: (T) -> Boolean,
) {
    private val items = ArrayDeque<T>()
    private val lock = Object()

    /** Returns true if something older had to be dropped to make room. */
    fun offer(item: T): Boolean =
        synchronized(lock) {
            var dropped = false
            if (items.size >= capacity) {
                val i = items.indexOfFirst { !isKey(it) }
                items.removeAt(if (i >= 0) i else 0)
                dropped = true
            }
            items.addLast(item)
            lock.notifyAll()
            dropped
        }

    fun poll(timeoutMs: Long): T? =
        synchronized(lock) {
            if (items.isEmpty() && timeoutMs > 0) {
                try {
                    lock.wait(timeoutMs)
                } catch (e: InterruptedException) {
                    Thread.currentThread().interrupt()
                }
            }
            items.removeFirstOrNull()
        }

    fun clear() = synchronized(lock) { items.clear() }

    val size: Int get() = synchronized(lock) { items.size }
}
