#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod media;

use config::Settings;
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{
    menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu},
    webview::NewWindowResponse,
    Manager, WebviewUrl, WebviewWindowBuilder,
};

struct Desktop {
    settings: Mutex<Settings>,
    path: PathBuf,
    client: reqwest::Client,
    connecting: AtomicBool,
    connection_error: Mutex<Option<String>>,
}

#[tauri::command]
fn connection_settings(state: tauri::State<'_, Desktop>) -> Result<serde_json::Value, String> {
    let settings = state
        .settings
        .lock()
        .map(|settings| settings.clone())
        .map_err(|_| "Settings are unavailable")?;
    Ok(serde_json::json!({
        "server": settings.server,
        "error": state.connection_error.lock().map_err(|_| "Settings are unavailable")?.take()
    }))
}

#[tauri::command]
async fn connect_server(app: tauri::AppHandle, address: String) -> Result<(), String> {
    let state = app.state::<Desktop>();
    if state.connecting.swap(true, Ordering::SeqCst) {
        return Err("A connection attempt is already in progress.".into());
    }
    let result = connect(&app, &address).await;
    state.connecting.store(false, Ordering::SeqCst);
    result
}

async fn connect(app: &tauri::AppHandle, address: &str) -> Result<(), String> {
    let server = config::server_url(address)?;
    let state = app.state::<Desktop>();
    config::probe(&state.client, &server).await?;
    let mut settings = state
        .settings
        .lock()
        .map_err(|_| "Settings are unavailable")?
        .clone();
    settings.server = Some(server.to_string());
    config::write(&state.path, &settings)?;
    *state
        .settings
        .lock()
        .map_err(|_| "Settings are unavailable")? = settings.clone();

    // Reusing a server keeps its cookies, cache, queue and localStorage. Separate
    // origins get separate profiles, including http/https and different ports.
    let profile = format!(
        "{:x}",
        Sha256::digest(server.origin().ascii_serialization().as_bytes())
    );
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("webview")
        .join(profile);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;

    // Explicit ACL: remote content can publish media state only. It cannot
    // change settings, open files, run programs, or use other Tauri APIs.
    let capability = serde_json::json!({
        "identifier": format!("media-{}", server.host_str().unwrap_or("server").replace(['.', ':', '[', ']'], "-")),
        "windows": ["main"], "local": false,
        "remote": { "urls": [format!("{}/*", server.origin().ascii_serialization())] },
        "permissions": ["allow-publish-media"]
    });
    app.add_capability(capability.to_string())
        .map_err(|error| error.to_string())?;
    let script = include_str!("media-bridge.js").replace(
        "__COSINE_ORIGIN__",
        &serde_json::to_string(&server.origin().ascii_serialization())
            .map_err(|error| error.to_string())?,
    );

    if let Some(window) = app.get_webview_window("main") {
        // destroy does not trigger the user-close handler that quits the app.
        window.destroy().map_err(|error| error.to_string())?;
    }
    let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(server))
        .title("cosine")
        .inner_size(
            settings.width.unwrap_or(1320.0).clamp(720.0, 3840.0),
            settings.height.unwrap_or(880.0).clamp(540.0, 2160.0),
        )
        .min_inner_size(720.0, 540.0)
        .maximized(settings.maximized)
        .background_color(tauri::window::Color(10, 10, 10, 255))
        .data_directory(directory)
        .initialization_script(script)
        .on_navigation(|url| matches!(url.scheme(), "http" | "https"))
        .on_new_window(move |url, _| {
            if matches!(url.scheme(), "http" | "https") {
                open_browser(&url);
            }
            NewWindowResponse::Deny
        });
    let window = builder
        .build()
        .map_err(|error| format!("Cannot open the player: {error}"))?;
    window
        .set_menu(player_menu(app).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    let handle = app.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
            save_window(&handle);
            handle.exit(0);
        }
    });
    if let Some(setup) = app.get_webview_window("setup") {
        let _ = setup.close();
    }
    Ok(())
}

fn player_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let server = MenuItem::with_id(app, "server", "Server connection…", true, Some("Ctrl+,"))?;
    let reload = MenuItem::with_id(app, "reload", "Reload", true, Some("Ctrl+R"))?;
    let browser = MenuItem::with_id(app, "browser", "Open in browser", true, None::<&str>)?;
    let fullscreen = MenuItem::with_id(app, "fullscreen", "Toggle fullscreen", true, Some("F11"))?;
    let zoom_in = MenuItem::with_id(app, "zoom-in", "Zoom in", true, Some("Ctrl+Plus"))?;
    let zoom_out = MenuItem::with_id(app, "zoom-out", "Zoom out", true, Some("Ctrl+-"))?;
    let zoom_reset = MenuItem::with_id(app, "zoom-reset", "Actual size", true, Some("Ctrl+0"))?;
    let separator = PredefinedMenuItem::separator(app)?;
    let about = PredefinedMenuItem::about(
        app,
        Some("About cosine"),
        Some(AboutMetadata {
            name: Some("cosine".into()),
            version: Some(env!("CARGO_PKG_VERSION").into()),
            authors: Some(vec!["cosine contributors".into()]),
            comments: Some("Your music. Your server.".into()),
            license: Some("AGPL-3.0-only".into()),
            website: Some("https://github.com/cosineaudio/cosine-desktop".into()),
            website_label: Some("cosineaudio/cosine-desktop".into()),
            icon: app.default_window_icon().cloned(),
            ..Default::default()
        }),
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, Some("Ctrl+Q"))?;
    let cosine = Submenu::with_items(
        app,
        "cosine",
        true,
        &[&server, &browser, &about, &separator, &quit],
    )?;
    let view = Submenu::with_items(
        app,
        "View",
        true,
        &[&reload, &fullscreen, &zoom_in, &zoom_out, &zoom_reset],
    )?;
    Menu::with_items(app, &[&cosine, &view])
}

fn open_browser(url: &url::Url) {
    // Called only by native menu/new-window callbacks, never by a web command.
    if let Err(error) = std::process::Command::new("xdg-open")
        .arg(url.as_str())
        .spawn()
    {
        eprintln!("Cannot open the browser: {error}");
    }
}

fn show_setup(app: &tauri::AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("setup") {
        window.unminimize()?;
        window.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, "setup", WebviewUrl::App("index.html".into()))
        .title("connect · cosine")
        .inner_size(520.0, 640.0)
        .min_inner_size(420.0, 560.0)
        .background_color(tauri::window::Color(10, 10, 10, 255))
        .build()?;
    Ok(())
}

fn save_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let state = app.state::<Desktop>();
        if let Ok(mut settings) = state.settings.lock() {
            settings.maximized = window.is_maximized().unwrap_or(false);
            if !settings.maximized && !window.is_fullscreen().unwrap_or(false) {
                if let (Ok(size), Ok(scale)) = (window.inner_size(), window.scale_factor()) {
                    let logical = size.to_logical::<f64>(scale);
                    settings.width = Some(logical.width);
                    settings.height = Some(logical.height);
                }
            }
            if let Err(error) = config::write(&state.path, &settings) {
                eprintln!("{error}");
            }
        };
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app
                .get_webview_window("main")
                .or_else(|| app.get_webview_window("setup"))
            {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            connection_settings,
            connect_server,
            media::publish_media
        ])
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("settings.json");
            let settings = config::read(&path).map_err(std::io::Error::other)?;
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent(concat!("cosine-desktop/", env!("CARGO_PKG_VERSION")))
                .build()?;
            app.manage(Desktop {
                settings: Mutex::new(settings),
                path,
                client,
                connecting: AtomicBool::new(false),
                connection_error: Mutex::new(None),
            });
            app.manage(media::initialize(app.handle()));
            let server = app
                .state::<Desktop>()
                .settings
                .lock()
                .map_err(|_| std::io::Error::other("Settings unavailable"))?
                .server
                .clone();
            if let Some(server) = server {
                // Returning launches never instantiate a connection WebView.
                // Only create one if the native connection check fails.
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = connect(&handle, &server).await {
                        if let Ok(mut message) = handle.state::<Desktop>().connection_error.lock() {
                            *message = Some(error);
                        }
                        if let Err(error) = show_setup(&handle) {
                            eprintln!("Cannot show connection screen: {error}");
                            handle.exit(1);
                        }
                    }
                });
            } else {
                show_setup(app.handle())?;
            }
            Ok(())
        })
        .on_menu_event(|app, event| {
            match event.id().as_ref() {
                "server" => {
                    if let Err(error) = show_setup(app) {
                        eprintln!("{error}");
                    }
                }
                "quit" => {
                    save_window(app);
                    app.exit(0);
                }
                id => {
                    if let Some(window) = app.get_webview_window("main") {
                        match id {
                            "reload" => {
                                let _ = window.eval("location.reload()");
                            }
                            "browser" => {
                                if let Ok(settings) = app.state::<Desktop>().settings.lock() {
                                    if let Some(server) = &settings.server {
                                        if let Ok(url) = url::Url::parse(server) {
                                            open_browser(&url);
                                        }
                                    }
                                }
                            }
                            "fullscreen" => {
                                let _ =
                                    window.set_fullscreen(!window.is_fullscreen().unwrap_or(false));
                            }
                            "zoom-in" | "zoom-out" | "zoom-reset" => {
                                // Zoom is kept in the window, without touching Cosine's layout or CSS.
                                let scale = match id {
                                    "zoom-in" => 1.1,
                                    "zoom-out" => 1.0 / 1.1,
                                    _ => 0.0,
                                };
                                let zoom = app.state::<Zoom>();
                                if let Ok(mut value) = zoom.0.lock() {
                                    *value = if scale == 0.0 {
                                        1.0
                                    } else {
                                        (*value * scale).clamp(0.5, 2.0)
                                    };
                                    let _ = window.set_zoom(*value);
                                };
                            }
                            _ => {}
                        }
                    }
                }
            }
        })
        .manage(Zoom(Mutex::new(1.0)))
        .run(tauri::generate_context!())
        .expect("Cannot start cosine desktop");
}

struct Zoom(Mutex<f64>);
