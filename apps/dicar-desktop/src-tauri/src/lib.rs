//! Native DiCar desktop bridge.

mod app_state;
mod channel_forwarder;
mod commands;
mod simulator_runtime;
mod window_guard;

pub use app_state::{AppState, BridgeErrorDto};
pub use channel_forwarder::{
    FrontendEvent, FrontendEventPayload, FrontendEventSequencer, FrontendSink, WindowCloseRequest,
};
pub use commands::{connect_core, list_serial_ports_core, EndpointDto};
#[cfg(any(target_env = "msvc", feature = "native-check"))]
pub use commands::{AccessProfileId, ParameterValueDto};
pub use simulator_runtime::{spawn_bundled_runtime, BundledSimulator};
pub use window_guard::{CloseDecision, CloseRequestOutcome, CloseResolution};

use window_guard::WindowCloseCoordinator;

#[cfg(any(target_env = "msvc", feature = "native-check"))]
pub fn command_handler() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static
{
    tauri::generate_handler![
        commands::connect,
        commands::list_serial_ports,
        commands::disconnect,
        commands::write_parameter,
        commands::commit_parameters,
        commands::revert_all,
        commands::undo_last,
        commands::set_telemetry_subscription,
        commands::set_paused,
        commands::add_marker,
        commands::select_access_profile,
        commands::get_snapshot,
        commands::open_core_channel,
        commands::close_core_channel,
        commands::resolve_window_close,
    ]
}

#[cfg(target_env = "msvc")]
pub fn run() {
    use tauri::Manager;

    let (simulator, state) = spawn_bundled_runtime()
        .unwrap_or_else(|error| panic!("failed to start DiCar runtime: {}", error.message));

    tauri::Builder::default()
        .manage(simulator)
        .manage(state)
        .invoke_handler(command_handler())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                match state.request_window_close() {
                    Ok(CloseRequestOutcome::Allow) => {}
                    Ok(CloseRequestOutcome::Prevented { .. }) | Err(_) => api.prevent_close(),
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run DiCar desktop shell");
}
