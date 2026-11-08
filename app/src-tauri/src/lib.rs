//! Family Tree desktop shell.

pub mod agent_access;
pub mod ai;
pub mod commands;
#[cfg(debug_assertions)]
pub mod e2e_hooks;
pub mod protocol;
pub mod settings_file;
pub mod state;

use state::AppState;
use tauri::Manager;

#[cfg(any(debug_assertions, feature = "spike"))]
mod spike {
    use std::time::{SystemTime, UNIX_EPOCH};
    use tauri::{AppHandle, Manager, Runtime};

    /// Writes the canvas spike result as `spike-<os>-<timestamp>.json` into the app data dir.
    #[tauri::command]
    pub fn spike_report<R: Runtime>(app: AppHandle<R>, json: String) -> Result<(), String> {
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        let name = format!("spike-{}-{secs}.json", std::env::consts::OS);
        std::fs::write(dir.join(name), json).map_err(|e| e.to_string())
    }

    pub fn handler<R: Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
        tauri::generate_handler![spike_report]
    }
}

pub fn run() {
    // The single-instance plugin must be registered first (its docs require it): a second launch
    // brings the running window forward instead of opening a second writer on the same tree.
    let builder = tauri::Builder::default();
    // A test copy (GT_E2E=1 in a debug build) must start next to the owner's running app instead of handing off to it.
    #[cfg(debug_assertions)]
    let single = !e2e_hooks::enabled();
    #[cfg(not(debug_assertions))]
    let single = true;
    let builder = if single {
        builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
    } else {
        builder
    };
    let builder = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            // A test run gets its own folder and keeps agent key hashes there, not in the keychain.
            #[cfg(debug_assertions)]
            let state = match e2e_hooks::config_dir_override() {
                Some(dir) => {
                    let hashes = agent_access::FileHashes::new(&dir);
                    let keys = ai::FileKey::new(&dir);
                    AppState::with_clock(
                        dir,
                        Box::new(gt_session::SessionClock::system),
                        Box::new(hashes),
                    )
                    .with_keys(std::sync::Arc::new(keys))
                }
                None => AppState::new(dir),
            };
            #[cfg(not(debug_assertions))]
            let state = AppState::new(dir);
            app.manage(state);
            // Scratch copies of a merge that did not finish hold a whole, unfiltered tree.
            // A test copy leaves the shared temp folder alone: the owner's own app may be in the middle of a merge.
            #[cfg(debug_assertions)]
            let sweep = !e2e_hooks::enabled();
            #[cfg(not(debug_assertions))]
            let sweep = true;
            if sweep {
                std::thread::spawn(|| {
                    gt_session::merge::remove_stale_scratch_now();
                });
            }
            let handle = app.handle().clone();
            std::thread::spawn(move || agent_access::resume(&handle));
            Ok(())
        });
    let builder = protocol::register(builder);
    let main = commands::handler();
    #[cfg(any(debug_assertions, feature = "spike"))]
    let builder = {
        let spike = spike::handler();
        #[cfg(debug_assertions)]
        let hooks = e2e_hooks::handler();
        builder.invoke_handler(move |inv| match inv.message.command() {
            "spike_report" => spike(inv),
            #[cfg(debug_assertions)]
            "e2e_hooks" => hooks(inv),
            _ => main(inv),
        })
    };
    #[cfg(not(any(debug_assertions, feature = "spike")))]
    let builder = builder.invoke_handler(main);
    #[allow(unused_mut)]
    let mut context = tauri::generate_context!();
    #[cfg(debug_assertions)]
    e2e_hooks::keep_windows_quiet(&mut context);
    builder
        .build(context)
        .expect("error while building Family Tree")
        .run(|app, event| {
            // Close the project cleanly: dropping it checkpoints the WAL into tree.db, so a
            // user who copies only that file loses nothing, and the lock file is released.
            // The server goes first, so no request meets a tree on its way out.
            if let tauri::RunEvent::Exit = event {
                let state = app.state::<AppState>();
                agent_access::shutdown(&state);
                commands::close_project_impl(&state);
            }
        });
}
