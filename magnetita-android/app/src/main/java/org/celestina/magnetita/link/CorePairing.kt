package org.celestina.magnetita.link

import uniffi.magnetita_mobile.PairPreview as CorePreview
import uniffi.magnetita_mobile.PairRefusal as CoreRefusal
import uniffi.magnetita_mobile.pairLinkAccepts
import uniffi.magnetita_mobile.previewPairing

/**
 * The core's reading of a pairing link, for [PairingConsent]: the parse, the
 * LAN rule and the refusal of this phone's own addresses all live in
 * `magnetita-mobile`, which reads the link with the very parse that pairs.
 */
object CorePairing {
    /** Whether a scanned text is a Magnetita pairing link at all. */
    fun accepts(text: String?): Boolean = text != null && pairLinkAccepts(text)

    /** `uri` read for the consent screen against this phone's own addresses. */
    fun preview(uri: String?, local: Set<String>): PairPreview {
        if (uri == null) return PairPreview.Refused(PairRefusal.NotMagnetita)
        return when (val read = previewPairing(uri, local.toList())) {
            is CorePreview.Offer -> PairPreview.Offer(PairOffer(read.offer.uri, read.offer.deviceId, read.offer.fingerprint, read.offer.addresses))
            is CorePreview.Refused -> PairPreview.Refused(
                when (read.reason) {
                    CoreRefusal.NOT_MAGNETITA -> PairRefusal.NotMagnetita
                    CoreRefusal.MALFORMED -> PairRefusal.Malformed
                    CoreRefusal.NO_ADDRESS -> PairRefusal.NoAddress
                    CoreRefusal.NOT_LAN -> PairRefusal.NotLan
                    CoreRefusal.THIS_PHONE -> PairRefusal.ThisPhone
                    CoreRefusal.LOCAL_UNKNOWN -> PairRefusal.LocalUnknown
                },
            )
        }
    }
}
