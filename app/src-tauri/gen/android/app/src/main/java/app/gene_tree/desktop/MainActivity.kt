package app.gene_tree.desktop

import android.graphics.Color
import android.os.Bundle
import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
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

  // The strips behind the status and navigation bars show the window's background: the page tells us its colour
  // (see `syncSystemBars` in mobile.ts) so they match the theme, light or dark, whatever the system theme is.
  override fun onWebViewCreate(webView: WebView) {
    webView.addJavascriptInterface(Bars(), "GeneTreeBars")
  }

  inner class Bars {
    /** The name of a picked document (what the person sees in the file picker), or "" when Android will not say. */
    @JavascriptInterface
    fun displayName(uri: String): String {
      return try {
        contentResolver.query(android.net.Uri.parse(uri), arrayOf(android.provider.OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
          if (c.moveToFirst()) c.getString(0) ?: "" else ""
        } ?: ""
      } catch (_: Exception) {
        ""
      }
    }

    @JavascriptInterface
    fun set(color: String, darkIcons: Boolean) {
      val c = try { Color.parseColor(color) } catch (_: IllegalArgumentException) { return }
      runOnUiThread {
        window.decorView.setBackgroundColor(c)
        findViewById<View>(android.R.id.content).setBackgroundColor(c)
        @Suppress("DEPRECATION")
        window.statusBarColor = c
        @Suppress("DEPRECATION")
        window.navigationBarColor = c
        val ctl = WindowCompat.getInsetsController(window, window.decorView)
        ctl.isAppearanceLightStatusBars = darkIcons
        ctl.isAppearanceLightNavigationBars = darkIcons
      }
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
