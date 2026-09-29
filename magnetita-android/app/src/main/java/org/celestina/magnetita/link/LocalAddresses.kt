package org.celestina.magnetita.link

import android.content.Context
import android.net.ConnectivityManager
import java.net.NetworkInterface

/**
 * This phone's own interface addresses as host literals, for
 * [PairPreview.of]: a pairing link naming one of them would be answered by
 * another app on this phone listening on every interface. Read from the
 * interfaces and from the active network's link properties; a source that
 * fails adds nothing. Local system calls only, no network traffic.
 */
object LocalAddresses {
    fun of(context: Context): Set<String> {
        val out = HashSet<String>()
        runCatching {
            NetworkInterface.getNetworkInterfaces()?.toList()?.forEach { nif ->
                nif.inetAddresses.toList().forEach { address -> address.hostAddress?.let(out::add) }
            }
        }
        runCatching {
            val connectivity = context.getSystemService(ConnectivityManager::class.java)
            connectivity?.getLinkProperties(connectivity.activeNetwork)?.linkAddresses?.forEach { link ->
                link.address.hostAddress?.let(out::add)
            }
        }
        return out
    }
}
