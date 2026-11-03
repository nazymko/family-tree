//! Gene Tree desktop shell.

#[cfg(any(debug_assertions, feature = "spike"))]
mod spike {
    use std::time::{SystemTime, UNIX_EPOCH};
    use tauri::{AppHandle, Manager};

    /// Writes the canvas spike result as `spike-<os>-<timestamp>.json` into the app data dir.
    #[tauri::command]
    pub fn spike_report(app: AppHandle, json: String) -> Result<(), String> {
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        let name = format!("spike-{}-{secs}.json", std::env::consts::OS);
        std::fs::write(dir.join(name), json).map_err(|e| e.to_string())
    }
}

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());
    #[cfg(any(debug_assertions, feature = "spike"))]
    let builder = builder.invoke_handler(tauri::generate_handler![spike::spike_report]);
    builder
        .run(tauri::generate_context!())
        .expect("error while running Gene Tree");
}
