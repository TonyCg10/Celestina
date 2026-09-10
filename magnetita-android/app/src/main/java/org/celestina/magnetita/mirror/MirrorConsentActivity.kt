package org.celestina.magnetita.mirror

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.media.projection.MediaProjectionManager
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import org.celestina.magnetita.link.MirrorOptions

/**
 * Asks the system's screen-capture consent and hands the answer to the
 * [MirrorService]. Android grants one capture per consent, so the desktop's
 * every start comes through here; the notification the link posts opens
 * it when the app is not in front.
 */
class MirrorConsentActivity : ComponentActivity() {
    private val ask = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        val data = result.data
        val options = MirrorService.readOptions(intent)
        if (result.resultCode == Activity.RESULT_OK && data != null && options != null) {
            MirrorService.start(this, data, options)
        } else {
            org.celestina.magnetita.link.LinkService.input { it.sendMirrorStop() }
        }
        finish()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val manager = getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
        ask.launch(manager.createScreenCaptureIntent())
    }

    companion object {
        fun intent(context: Context, options: MirrorOptions): Intent =
            MirrorService.putOptions(Intent(context, MirrorConsentActivity::class.java), options)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }
}
