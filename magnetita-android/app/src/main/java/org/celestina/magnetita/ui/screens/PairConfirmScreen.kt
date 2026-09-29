package org.celestina.magnetita.ui.screens

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.celestina.magnetita.R
import org.celestina.magnetita.link.PairOffer
import org.celestina.magnetita.link.PairRefusal
import org.celestina.magnetita.link.PairingState
import org.celestina.magnetita.ui.components.Canvas
import org.celestina.magnetita.ui.components.Group
import org.celestina.magnetita.ui.components.GroupRow
import org.celestina.magnetita.ui.components.Header

/**
 * The consent in front of pairing: the desktop a link names (its id, its
 * certificate's fingerprint and the addresses the phone would dial) and
 * one press to pair or to cancel; or, for a refused link, why. Nothing
 * pairs from this screen but the confirm button.
 */
@Composable
fun PairConfirmScreen(state: PairingState, onPair: (PairOffer) -> Unit, onDismiss: () -> Unit) {
    Canvas {
        Column(modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
            Header(title = stringResource(R.string.title_scan), progress = 1f)
            Spacer(Modifier.height(8.dp))
            when (state) {
                is PairingState.Confirming -> {
                    val offer = state.offer
                    // Taps in the first moments are ignored, counted from the offer and
                    // from every return of the window's focus: a tap meant for what was
                    // on screen before, or for a window laid over this one, must not pair.
                    val focused = LocalWindowInfo.current.isWindowFocused
                    var armed by remember(offer, focused) { mutableStateOf(false) }
                    LaunchedEffect(offer, focused) {
                        if (focused) {
                            kotlinx.coroutines.delay(ARM_MS)
                            armed = true
                        }
                    }
                    Group {
                        GroupRow(title = stringResource(R.string.label_desktop), detail = offer.deviceId)
                        GroupRow(title = stringResource(R.string.label_fingerprint), detail = offer.fingerprint)
                        GroupRow(title = stringResource(R.string.label_address), detail = offer.addresses.joinToString(", "), last = true)
                    }
                    Spacer(Modifier.height(20.dp))
                    Text(
                        stringResource(R.string.pair_confirm_hint),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 32.dp),
                    )
                    Spacer(Modifier.height(20.dp))
                    Button(onClick = { onPair(offer) }, enabled = armed, modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp)) {
                        Text(stringResource(R.string.action_pair))
                    }
                    TextButton(onClick = onDismiss, modifier = Modifier.padding(horizontal = 20.dp)) {
                        Text(stringResource(R.string.action_cancel))
                    }
                }
                is PairingState.Refused -> {
                    Text(
                        stringResource(
                            when (state.reason) {
                                PairRefusal.NotMagnetita -> R.string.scan_not_magnetita
                                PairRefusal.Malformed -> R.string.pair_refused_malformed
                                PairRefusal.NoAddress -> R.string.pair_refused_no_address
                                PairRefusal.NotLan -> R.string.pair_refused_not_lan
                                PairRefusal.ThisPhone -> R.string.pair_refused_this_phone
                                PairRefusal.Conflict -> R.string.pair_refused_conflict
                                PairRefusal.Expired -> R.string.pair_refused_expired
                                PairRefusal.LocalUnknown -> R.string.pair_refused_local_unknown
                            },
                        ),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(horizontal = 32.dp),
                    )
                    Spacer(Modifier.height(12.dp))
                    TextButton(onClick = onDismiss, modifier = Modifier.padding(horizontal = 20.dp)) {
                        Text(stringResource(R.string.action_back))
                    }
                }
                PairingState.Idle -> {}
            }
        }
    }
}

/** How long the confirm button stays disabled after an offer appears. */
private const val ARM_MS = 500L
