mod commands;
mod services;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(crate::services::recorder::RecorderState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            services::tray::build(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window must not take the menu bar icon with it.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::media::check_dependencies,
            commands::media::default_whisper_model,
            commands::media::probe_video,
            commands::media::run_job,
            commands::media::transcribe_file,
            commands::media::combine_transcripts,
            commands::media::reveal_in_file_manager,
            commands::media::open_output_folder,
            commands::recording::list_audio_inputs,
            commands::recording::set_audio_input,
            commands::recording::toggle_recording,
            commands::recording::save_recording,
        ])
        .build(tauri::generate_context!())
        .expect("failed to start the app")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit) {
                services::tray::shutdown(app);
            }
        });
}
