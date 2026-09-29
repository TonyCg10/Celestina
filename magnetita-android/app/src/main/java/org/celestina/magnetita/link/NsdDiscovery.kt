package org.celestina.magnetita.link

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import kotlinx.coroutines.delay
import uniffi.magnetita_mobile.Resolution
import uniffi.magnetita_mobile.rankAdvertised

/**
 * `_magnetita._udp` through Android's `NsdManager`, one bounded browse per
 * call: discover for a few seconds and resolve each service. Which of the
 * results to dial, in which order, is the core's discovery rule
 * (`rankAdvertised`), the one the desktop's link applies. The multicast
 * lock is held only while browsing, which is what the battery wants.
 */
class NsdDiscovery(context: Context, private val browseMs: Long = 4_000) : Discovery {
    private val nsd = context.getSystemService(Context.NSD_SERVICE) as NsdManager
    private val wifi = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager

    override suspend fun browse(): List<Advertised> {
        val lock = wifi.createMulticastLock("magnetita-browse").apply { setReferenceCounted(false); acquire() }
        try {
            val found = ArrayList<Resolution>()
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
                            val host = info.host?.hostAddress ?: return
                            val port = info.port.takeIf { it in 1..65535 } ?: return
                            synchronized(found) {
                                if (found.size < MAX_RESOLVED) found += Resolution(info.serviceName.orEmpty(), host, port.toUShort())
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
            val seen = synchronized(found) { found.toList() }
            return rankAdvertised(seen).map { Advertised(it.deviceId, it.address) }
        } finally {
            runCatching { lock.release() }
        }
    }

    companion object {
        const val SERVICE_TYPE = "_magnetita._udp."

        /** Most resolutions one browse keeps; a flood of advertisements stops here. */
        private const val MAX_RESOLVED = 64
    }
}
