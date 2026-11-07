// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.nearby

import android.app.Activity
import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/**
 * Screens on the same network for Dart (spec 14, native-apps.md 4.1), MethodChannel
 * `de.cometcast/nearby`. Browses `_cometcast._tcp` with [NsdManager] and hands the raw records to
 * Dart, which passes them to the Rust core; the core checks and cleans every field, keeps the
 * list and the 30 s expiry (crates/cc-discovery, nearby.rs). Nothing is decided here.
 *
 * From Dart:
 * - `permission` -> "granted" (browsing needs no permission on Android; a future local-network
 *   permission would be reported here, like iOS).
 * - `start` / `stop`: browse, with a multicast lock held while browsing (see [acquireLock]).
 * - `hasNetwork` -> Boolean: a Wi-Fi or Ethernet network exists (no `TRANSPORT_WIFI` or
 *   `TRANSPORT_ETHERNET` means mobile data only: "No local network").
 * - `openSettings` -> false (no permission page).
 * To Dart: `seen` {instance, addresses, txt}, `gone` instance, `reset`, `network` Boolean.
 *
 * Refresh: `NsdManager` reports a service when it appears or changes, but a display refreshes its
 * record without changing it and the core drops a row after 30 s without a refresh. So every
 * [REFRESH_MS] the services that are still known are reported again; below API 34 each is
 * resolved again first (a service that does not answer is reported as gone). Resolution below
 * API 34 is one at a time (`resolveService` fails with FAILURE_ALREADY_ACTIVE otherwise), so a
 * queue is used; from API 34 `registerServiceInfoCallback` delivers updates.
 *
 * UNCERTAIN: never run on a device (docs/android.md). Whether `NsdManager` sees a service that
 * died without a goodbye within 30 s, and whether the multicast lock is needed at all, are not
 * known; the records announced by `mdns-sd` carry long TTLs (docs/discovery.md).
 */
class NearbyBridge(
    private val activity: Activity,
    engine: FlutterEngine,
) : MethodChannel.MethodCallHandler {
    private val channel = MethodChannel(engine.dartExecutor.binaryMessenger, CHANNEL)
    private val main = Handler(Looper.getMainLooper())
    private val appContext = activity.applicationContext
    private val nsd = appContext.getSystemService(Context.NSD_SERVICE) as NsdManager
    private val wifi = appContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager
    private val connectivity = appContext.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager

    private var discovery: NsdManager.DiscoveryListener? = null
    private var lock: WifiManager.MulticastLock? = null
    private var networkCallback: ConnectivityManager.NetworkCallback? = null
    private var lastNetwork: Boolean? = null

    /** Service name -> the last record reported to Dart (main thread only). */
    private val known = LinkedHashMap<String, Record>()
    private val queue = ArrayDeque<NsdServiceInfo>()
    private var resolving = false
    private val callbacks = HashMap<String, Any>()
    private var generation = 0

    private val refresh =
        object : Runnable {
            override fun run() {
                refreshKnown()
                if (discovery != null) main.postDelayed(this, REFRESH_MS)
            }
        }

    init {
        channel.setMethodCallHandler(this)
        watchNetwork()
    }

    override fun onMethodCall(
        call: MethodCall,
        result: MethodChannel.Result,
    ) {
        when (call.method) {
            "permission" -> result.success("granted")
            "start" -> {
                start()
                result.success(null)
            }
            "stop" -> {
                stop()
                result.success(null)
            }
            "hasNetwork" -> result.success(hasLocalNetwork())
            "openSettings" -> result.success(false)
            else -> result.notImplemented()
        }
    }

    // --- browsing -----------------------------------------------------------------------

    private fun start() {
        if (discovery != null) return
        generation++
        val mine = generation
        acquireLock()
        val listener =
            object : NsdManager.DiscoveryListener {
                override fun onDiscoveryStarted(serviceType: String) {}

                override fun onServiceFound(info: NsdServiceInfo) {
                    main.post { if (mine == generation) resolve(info) }
                }

                override fun onServiceLost(info: NsdServiceInfo) {
                    main.post { if (mine == generation) lost(info.serviceName) }
                }

                override fun onDiscoveryStopped(serviceType: String) {}

                override fun onStartDiscoveryFailed(
                    serviceType: String,
                    errorCode: Int,
                ) {
                    // The core shows "nothing found"; there is nothing better to say.
                    main.post { if (mine == generation) discovery = null }
                }

                override fun onStopDiscoveryFailed(
                    serviceType: String,
                    errorCode: Int,
                ) {}
            }
        discovery = listener
        try {
            nsd.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, listener)
        } catch (e: Exception) {
            discovery = null
            releaseLock()
            return
        }
        main.postDelayed(refresh, REFRESH_MS)
    }

    private fun stop() {
        generation++
        main.removeCallbacks(refresh)
        discovery?.let {
            try {
                nsd.stopServiceDiscovery(it)
            } catch (e: Exception) {
                // Already stopped.
            }
        }
        discovery = null
        if (Build.VERSION.SDK_INT >= 34) {
            for (cb in callbacks.values) {
                try {
                    nsd.unregisterServiceInfoCallback(cb as NsdManager.ServiceInfoCallback)
                } catch (e: Exception) {
                    // Not registered any more.
                }
            }
        }
        callbacks.clear()
        queue.clear()
        resolving = false
        known.clear()
        releaseLock()
    }

    private fun resolve(info: NsdServiceInfo) {
        if (Build.VERSION.SDK_INT >= 34) {
            if (callbacks.containsKey(info.serviceName)) return
            val mine = generation
            val cb =
                object : NsdManager.ServiceInfoCallback {
                    override fun onServiceInfoCallbackRegistrationFailed(errorCode: Int) {}

                    override fun onServiceUpdated(updated: NsdServiceInfo) {
                        main.post { if (mine == generation) report(updated) }
                    }

                    override fun onServiceLost() {
                        main.post { if (mine == generation) lost(info.serviceName) }
                    }

                    override fun onServiceInfoCallbackUnregistered() {}
                }
            callbacks[info.serviceName] = cb
            try {
                nsd.registerServiceInfoCallback(info, appContext.mainExecutor, cb)
            } catch (e: Exception) {
                callbacks.remove(info.serviceName)
            }
            return
        }
        if (queue.none { it.serviceName == info.serviceName }) queue.add(info)
        pump()
    }

    @Suppress("DEPRECATION")
    private fun pump() {
        if (resolving) return
        val next = queue.removeFirstOrNull() ?: return
        resolving = true
        val mine = generation
        nsd.resolveService(
            next,
            object : NsdManager.ResolveListener {
                override fun onResolveFailed(
                    info: NsdServiceInfo,
                    errorCode: Int,
                ) {
                    main.post {
                        resolving = false
                        // A service that was known and does not answer any more is gone.
                        if (mine == generation && known.containsKey(info.serviceName)) lost(info.serviceName)
                        pump()
                    }
                }

                override fun onServiceResolved(info: NsdServiceInfo) {
                    main.post {
                        resolving = false
                        if (mine == generation) report(info)
                        pump()
                    }
                }
            },
        )
    }

    private fun report(info: NsdServiceInfo) {
        val record = Record.of(info)
        known[info.serviceName] = record
        push(record)
    }

    private fun push(r: Record) {
        channel.invokeMethod(
            "seen",
            mapOf("instance" to r.instance, "addresses" to r.addresses, "txt" to r.txt),
        )
    }

    private fun lost(name: String) {
        known.remove(name)
        if (Build.VERSION.SDK_INT >= 34) {
            (callbacks.remove(name) as? NsdManager.ServiceInfoCallback)?.let {
                try {
                    nsd.unregisterServiceInfoCallback(it)
                } catch (e: Exception) {
                    // Gone already.
                }
            }
        }
        channel.invokeMethod("gone", name)
    }

    /** Every [REFRESH_MS]: the services that are still known are reported again (see class doc). */
    private fun refreshKnown() {
        if (Build.VERSION.SDK_INT >= 34) {
            for (r in known.values) push(r)
            return
        }
        for (name in known.keys.toList()) {
            val info = NsdServiceInfo()
            info.serviceName = name
            info.serviceType = SERVICE_TYPE
            if (queue.none { it.serviceName == name }) queue.add(info)
        }
        pump()
    }

    // --- multicast lock -----------------------------------------------------------------

    /**
     * Held while browsing. `NsdManager` talks to the system's mDNS daemon, which should not need
     * it, but some devices filter multicast on Wi-Fi while the app is in use. Needs
     * CHANGE_WIFI_MULTICAST_STATE (a normal permission). UNCERTAIN: not tried on a device.
     */
    private fun acquireLock() {
        if (lock?.isHeld == true) return
        lock =
            wifi?.createMulticastLock("cometcast-nearby")?.also {
                it.setReferenceCounted(false)
                try {
                    it.acquire()
                } catch (e: SecurityException) {
                    // The permission is missing: browse without the lock.
                }
            }
    }

    private fun releaseLock() {
        try {
            if (lock?.isHeld == true) lock?.release()
        } catch (e: Exception) {
            // Nothing to release.
        }
        lock = null
    }

    // --- the network --------------------------------------------------------------------

    private fun hasLocalNetwork(): Boolean =
        connectivity.allNetworks.any { n ->
            val caps = connectivity.getNetworkCapabilities(n)
            caps != null &&
                (
                    caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) ||
                        caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET)
                )
        }

    private fun watchNetwork() {
        val request =
            NetworkRequest
                .Builder()
                .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
                .addTransportType(NetworkCapabilities.TRANSPORT_ETHERNET)
                .build()
        val cb =
            object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) = notifyNetwork()

                override fun onLost(network: Network) = notifyNetwork()
            }
        networkCallback = cb
        try {
            connectivity.registerNetworkCallback(request, cb)
        } catch (e: Exception) {
            networkCallback = null
        }
    }

    private fun notifyNetwork() {
        main.post {
            val now = hasLocalNetwork()
            if (now != lastNetwork) {
                lastNetwork = now
                channel.invokeMethod("network", now)
            }
        }
    }

    /** The activity goes away: stop browsing and drop the callback. */
    fun release() {
        stop()
        networkCallback?.let {
            try {
                connectivity.unregisterNetworkCallback(it)
            } catch (e: Exception) {
                // Not registered.
            }
        }
        networkCallback = null
    }

    /** What is reported to Dart for one service. */
    private data class Record(
        val instance: String,
        val addresses: List<String>,
        val txt: Map<String, String>,
    ) {
        companion object {
            fun of(info: NsdServiceInfo): Record {
                val addresses =
                    if (Build.VERSION.SDK_INT >= 34) {
                        info.hostAddresses.mapNotNull { it.hostAddress }
                    } else {
                        @Suppress("DEPRECATION")
                        listOfNotNull(info.host?.hostAddress)
                    }
                val txt = LinkedHashMap<String, String>()
                for ((k, v) in info.attributes.entries.take(MAX_TXT_ENTRIES)) {
                    if (v == null || v.size > MAX_TXT_VALUE) continue
                    txt[k] = String(v, Charsets.UTF_8)
                }
                return Record(info.serviceName, addresses.take(MAX_ADDRESSES), txt)
            }
        }
    }

    companion object {
        private const val CHANNEL = "de.cometcast/nearby"
        private const val SERVICE_TYPE = "_cometcast._tcp."
        private const val REFRESH_MS = 10_000L
        private const val MAX_TXT_ENTRIES = 32
        private const val MAX_TXT_VALUE = 255
        private const val MAX_ADDRESSES = 8
    }
}
