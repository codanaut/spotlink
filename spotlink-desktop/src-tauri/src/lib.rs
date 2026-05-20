use tauri::{State, Manager, Emitter};
use tokio::sync::{mpsc, broadcast};
use spotlink_core::{EngineCommand, EngineEvent, run_engine};

// Wrap our engine's command pipe in a struct so Tauri can store it safely in its global state.
struct AppEngineState {
    command_sender: mpsc::Sender<EngineCommand>,
}

// --- TAURI COMMANDS (Invoked from Svelte JavaScript) ---

// Track callsign
#[tauri::command]
async fn track_callsign(callsign: String, state: State<'_, AppEngineState>) -> Result<(), String> {
    // Drop a command into the inbound engine pipe
    state.command_sender
        .send(EngineCommand::StartTracking { callsign })
        .await
        .map_err(|e| e.to_string())
}

// Stop tracking callsign
#[tauri::command]
async fn stop_tracking(state: State<'_, AppEngineState>) -> Result<(), String> {
    state.command_sender
        .send(EngineCommand::StopTracking)
        .await
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 1. Allocate our asynchronous communications pipelines
            let (command_sender, command_receiver) = mpsc::channel::<EngineCommand>(32);
            let (event_broadcaster, mut event_receiver) = broadcast::channel::<EngineEvent>(100);

            // 2. Register the sender channel into Tauri's managed memory state
            app.manage(AppEngineState { command_sender });

            // 3. Spawn the background processing engine on Tauri's async executor loop
            tauri::async_runtime::spawn(async move {
                run_engine(command_receiver, event_broadcaster).await;
            });

            // 4. Capture the main application window handle
            let window = app.get_webview_window("main").unwrap();

            // 5. Spawn an independent listener that listens for discovered matches out of the engine,
            // and emits them instantly as an event over the fence to the Svelte Frontend.
            tauri::async_runtime::spawn(async move {
                while let Ok(event) = event_receiver.recv().await {
                    match event {
                        EngineEvent::MatchUpdate(discovered_match) => {
                            let _ = window.emit("new-match", discovered_match);
                        }
                        EngineEvent::GlobalTimeout => {
                            let _ = window.emit("clear-matches", ());
                        }
                        EngineEvent::StatsUpdate(current_stats) => {
                            let _ = window.emit("stats-update", current_stats);
                        }
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![track_callsign, stop_tracking])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}