package org.celestina.magnetita.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.dp
import org.celestina.magnetita.R
import org.celestina.magnetita.control.TrackpadMath
import org.celestina.magnetita.ui.components.Canvas
import org.celestina.magnetita.ui.components.Group
import org.celestina.magnetita.ui.components.GroupRow
import org.celestina.magnetita.ui.components.Header

/** What the control screen sends; the service turns it into wire input. */
interface Remote {
    fun move(dx: Int, dy: Int)
    fun button(button: Int, pressed: Boolean)
    fun scroll(dx: Int, dy: Int)
    fun key(code: Int, pressed: Boolean)
    fun type(text: String)
    fun run(id: Int)
}

/**
 * The phone as trackpad, keyboard and command deck. The pad is the top of
 * the page: one finger moves, a tap clicks, two fingers scroll, a long
 * press is the right button. Below it the text field types straight to
 * the desktop, a row of keys covers what a field cannot, and the
 * registered commands run by name.
 */
@Composable
fun ControlScreen(remote: Remote, commands: List<Pair<Int, String>>, commandNote: String, connected: Boolean) {
    val math = remember { TrackpadMath() }
    Canvas {
        Column(modifier = Modifier.fillMaxSize()) {
            Header(title = stringResource(R.string.title_control), subtitle = if (connected) stringResource(R.string.chip_connected) else null, progress = 1f)
            Spacer(Modifier.height(8.dp))
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp)
                    .height(260.dp)
                    .background(MaterialTheme.colorScheme.surface, RoundedCornerShape(26.dp))
                    .pointerInput(Unit) {
                        awaitEachGesture {
                            val first = awaitFirstDown()
                            var moved = false
                            var fingers = 1
                            val start = System.currentTimeMillis()
                            var last = first.position
                            while (true) {
                                val event = awaitPointerEvent()
                                val pressed = event.changes.filter { it.pressed }
                                if (pressed.isEmpty()) break
                                fingers = maxOf(fingers, pressed.size)
                                val current = pressed.first().position
                                val dx = current.x - last.x
                                val dy = current.y - last.y
                                if (kotlin.math.abs(dx) + kotlin.math.abs(dy) > 0.5f) {
                                    moved = true
                                    if (fingers >= 2) {
                                        val steps = math.scroll(dy)
                                        if (steps != 0) remote.scroll(0, steps)
                                    } else {
                                        val (mx, my) = math.move(dx, dy)
                                        if (mx != 0 || my != 0) remote.move(mx, my)
                                    }
                                }
                                last = current
                                event.changes.forEach { it.consume() }
                            }
                            if (!moved) {
                                val held = System.currentTimeMillis() - start
                                val button = if (fingers >= 2 || held > 400) 1 else 0
                                remote.button(button, true)
                                remote.button(button, false)
                            }
                        }
                    },
                contentAlignment = Alignment.Center,
            ) {
                Text(stringResource(R.string.trackpad_hint), color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodyMedium)
            }
            Spacer(Modifier.height(16.dp))
            var typed by remember { mutableStateOf("") }
            Group {
                BasicTextField(
                    value = typed,
                    onValueChange = { next ->
                        val (added, removed) = TrackpadMath.diff(typed, next)
                        repeat(removed) { remote.key(TrackpadMath.KEY_BACKSPACE, true); remote.key(TrackpadMath.KEY_BACKSPACE, false) }
                        if (added.isNotEmpty()) remote.type(added)
                        // The field forgets what it sent: the desktop holds the text.
                        typed = if (next.length > 200) "" else next
                    },
                    textStyle = TextStyle(color = MaterialTheme.colorScheme.onSurface),
                    cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 14.dp),
                    decorationBox = { inner ->
                        if (typed.isEmpty()) Text(stringResource(R.string.keyboard_hint), color = MaterialTheme.colorScheme.onSurfaceVariant)
                        inner()
                    },
                )
                Row(modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    KeyButton("Esc") { remote.tap(TrackpadMath.KEY_ESC) }
                    KeyButton("Tab") { remote.tap(TrackpadMath.KEY_TAB) }
                    KeyButton("←") { remote.tap(TrackpadMath.KEY_LEFT) }
                    KeyButton("↑") { remote.tap(TrackpadMath.KEY_UP) }
                    KeyButton("↓") { remote.tap(TrackpadMath.KEY_DOWN) }
                    KeyButton("→") { remote.tap(TrackpadMath.KEY_RIGHT) }
                    KeyButton("⏎") { remote.tap(TrackpadMath.KEY_ENTER) }
                }
            }
            Spacer(Modifier.height(16.dp))
            Group {
                if (commands.isEmpty()) {
                    GroupRow(title = stringResource(R.string.label_commands), detail = stringResource(R.string.detail_no_commands), last = true)
                } else {
                    commands.forEachIndexed { i, (id, name) ->
                        GroupRow(
                            title = name,
                            detail = if (i == commands.lastIndex && commandNote.isNotBlank()) commandNote else null,
                            last = i == commands.lastIndex,
                            trailing = { FilledTonalButton(onClick = { remote.run(id) }) { Text(stringResource(R.string.action_run)) } },
                        )
                    }
                }
            }
            Spacer(Modifier.height(120.dp))
        }
    }
}

private fun Remote.tap(code: Int) {
    key(code, true)
    key(code, false)
}

@Composable
private fun KeyButton(label: String, onClick: () -> Unit) {
    FilledTonalButton(onClick = onClick) { Text(label) }
}
