package org.celestina.magnetita.link

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import kotlinx.coroutines.delay
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull
import java.net.Inet4Address
import kotlin.coroutines.resume

/**
 * `_magnetita._udp` through Android's `NsdManager`, one bounded browse per
 * call: discover for a few seconds, resolve each service, prefer IPv4.
 * The multicast lock is held only while browsing, which is what the
 * battery wants.
 */
class NsdDiscovery(context: Context, private val browseMs: Long = 4_000) : Discovery {
    private val nsd = context.getSystemService(Context.NSD_SERVICE) as NsdManager
    private val wifi = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager

    override suspend fun browse(): List<Advertised> {
        val lock = wifi.createMulticastLock("magnetita-browse").apply { setReferenceCounted(false); acquire() }
        try {
            val found = LinkedHashMap<String, Advertised>()
            val listener = object : NsdManager.DiscoveryListener {
                override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {}
                override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {}
                override fun onDiscoveryStarted(serviceType: String) {}
                override fun onDiscoveryStopped(serviceType: String) {}
                override fun onServiceLost(serviceInfo: NsdServiceInfo) {}
                override fun onServiceFound(serviceInfo: NsdServiceInfo) {
                    nsd.resolveService(serviceInfo, object : NsdManager.ResolveListener {
                        override fun onResolveFailed(serviceInfo: NsdServiceInfo, errorCode: Int) {}
                        override fun onServiceResolved(info: NsdServiceInfo) {
                            val host = info.host ?: return
                            if (host.isLoopbackAddress || host.isLinkLocalAddress) return
                            val text = if (host is Inet4Address) host.hostAddress else "[${host.hostAddress}]"
                            val id = info.serviceName
                            if (id.length <= 64 && id.all { it.isLetterOrDigit() }) {
                                synchronized(found) { found.putIfAbsent("$id@$text", Advertised(id, "$text:${info.port}")) }
                            }
                        }
                    })
                }
            }
            nsd.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, listener)
            try {
                delay(browseMs)
            } finally {
                runCatching { nsd.stopServiceDiscovery(listener) }
            }
            // IPv4 first: the daemon binds both, and v4 is what the QR names.
            return synchronized(found) { found.values.sortedBy { if (it.address.startsWith("[")) 1 else 0 } }
        } finally {
            runCatching { lock.release() }
        }
    }

    companion object {
        const val SERVICE_TYPE = "_magnetita._udp."
    }
}
