package app.gene_tree.desktop

import android.os.Bundle

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
  }

  // Back sends the app to the background instead of finishing the activity. Finishing it makes the Rust runtime
  // exit the process while its worker threads still run, and the process then aborts in a C++ destructor
  // ("pthread_mutex_lock called on a destroyed mutex"). The system ends a background process by itself, and the
  // open tree is checkpointed when the app is sent away (see `checkpoint_open_tree` in mobile.rs).
  @Deprecated("Back is handled by the Wry callback, which ends here.")
  override fun onBackPressed() {
    moveTaskToBack(true)
  }
}
