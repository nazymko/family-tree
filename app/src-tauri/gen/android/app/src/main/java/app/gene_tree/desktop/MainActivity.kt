package app.gene_tree.desktop

import android.os.Bundle
import android.view.View
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    // The page sits between the status bar and the navigation bar, and above the keyboard: from Android 15 the
    // window is edge to edge, and a WebView does not report those insets to the page's env(safe-area-inset-*).
    val root = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(root) { view, insets ->
      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or
          WindowInsetsCompat.Type.displayCutout() or
          WindowInsetsCompat.Type.ime()
      )
      view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
      insets
    }
  }

  // Back sends the app to the background instead of finishing the activity: the usual behaviour of an app that
  // holds a working document, and it avoids the slow cold start after a stray back press and the teardown of the
  // Rust runtime that a finished activity triggers (see `end_process` in mobile.rs).
  @Deprecated("Back is handled by the Wry callback, which ends here.")
  override fun onBackPressed() {
    moveTaskToBack(true)
  }
}
