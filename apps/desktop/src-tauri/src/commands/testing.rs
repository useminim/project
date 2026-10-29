//! Outils de test des commandes : application simulée (runtime de test de
//! Tauri) avec la configuration et la capability réelles.

use serde_json::Value;
use tauri::Builder;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response};
use tauri::webview::{InvokeRequest, WebviewWindow, WebviewWindowBuilder};

/// Construit l'application à partir de `builder` (états déjà ajoutés) et ouvre
/// la fenêtre principale, soumise à la capability `default`.
pub fn mock_app(
    builder: Builder<MockRuntime>,
) -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let app = builder
        .invoke_handler(super::handler())
        // `test = true` : sans l'Info.plist embarqué sous macOS, déjà défini
        // par `run()` dans cette bibliothèque (symbole en double sinon).
        .build(tauri::generate_context!(test = true))
        .unwrap();
    let window = WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::default())
        .build()
        .unwrap();
    (app, window)
}

/// Appelle la commande `cmd` depuis la fenêtre, comme `invoke` côté interface.
pub fn call(window: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
    get_ipc_response(
        window,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: if cfg!(windows) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_owned(),
        },
    )
    .map(|response| response.deserialize::<Value>().unwrap())
}
