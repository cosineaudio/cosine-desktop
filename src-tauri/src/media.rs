use serde::Deserialize;
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use std::{sync::Mutex, time::Duration};
use tauri::{AppHandle, Manager, WebviewWindow};

pub struct Media(pub Mutex<MediaState>);

pub struct MediaState {
    controls: Option<MediaControls>,
    metadata: Option<(String, String, String, Duration)>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    title: String,
    artist: String,
    album: String,
    state: String,
    duration: f64,
    position: f64,
}

pub fn initialize(app: &AppHandle) -> Media {
    let result = (|| {
        let mut controls = MediaControls::new(PlatformConfig {
            dbus_name: "cosine",
            display_name: "cosine",
            hwnd: None,
        })?;
        let app = app.clone();
        controls.attach(move |event| {
            let (action, seconds) = match event {
                MediaControlEvent::Play => ("play", 0.0),
                MediaControlEvent::Pause => ("pause", 0.0),
                MediaControlEvent::Toggle => ("toggle", 0.0),
                MediaControlEvent::Next => ("nexttrack", 0.0),
                MediaControlEvent::Previous => ("previoustrack", 0.0),
                MediaControlEvent::Stop => ("stop", 0.0),
                MediaControlEvent::SetPosition(position) => ("seekto", position.0.as_secs_f64()),
                MediaControlEvent::SeekBy(direction, duration) => (
                    if direction == SeekDirection::Forward {
                        "seekforward"
                    } else {
                        "seekbackward"
                    },
                    duration.as_secs_f64(),
                ),
                MediaControlEvent::Raise => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                    return;
                }
                MediaControlEvent::Quit => {
                    app.exit(0);
                    return;
                }
                _ => return,
            };
            if let Some(window) = app.get_webview_window("main") {
                // Only calls the bridge installed for this server; no page-to-native event subscription.
                let _ = window.eval(format!(
                    "window.__cosineDesktopMedia?.({action:?}, {seconds})"
                ));
            }
        })?;
        Ok::<_, souvlaki::Error>(controls)
    })();
    match result {
        Ok(controls) => Media(Mutex::new(MediaState {
            controls: Some(controls),
            metadata: None,
        })),
        Err(error) => {
            eprintln!("Linux media controls are unavailable: {error}");
            Media(Mutex::new(MediaState {
                controls: None,
                metadata: None,
            }))
        }
    }
}

#[tauri::command]
pub fn publish_media(
    window: WebviewWindow,
    app: AppHandle,
    snapshot: Snapshot,
) -> Result<(), String> {
    let settings = app.state::<crate::Desktop>();
    let server = settings
        .settings
        .lock()
        .map_err(|_| "Settings are unavailable")?
        .server
        .clone()
        .ok_or("No server selected")?;
    let actual = window.url().map_err(|error| error.to_string())?;
    if window.label() != "main"
        || actual.origin()
            != url::Url::parse(&server)
                .map_err(|_| "Invalid server")?
                .origin()
    {
        return Err("Media updates are only accepted from the selected server.".into());
    }
    if snapshot.title.len() > 4096 || snapshot.artist.len() > 4096 || snapshot.album.len() > 4096 {
        return Err("Media metadata is too large.".into());
    }
    let time = |value: f64| {
        Duration::from_secs_f64(if value.is_finite() {
            value.clamp(0.0, 31_536_000.0)
        } else {
            0.0
        })
    };
    let media = app.state::<Media>();
    let mut guard = media
        .0
        .lock()
        .map_err(|_| "Media controls are unavailable")?;
    let metadata = (
        snapshot.title.clone(),
        snapshot.artist.clone(),
        snapshot.album.clone(),
        time(snapshot.duration),
    );
    let changed = guard.metadata.as_ref() != Some(&metadata);
    if let Some(controls) = guard.controls.as_mut() {
        if changed {
            controls
                .set_metadata(MediaMetadata {
                    title: Some(&snapshot.title),
                    artist: Some(&snapshot.artist),
                    album: Some(&snapshot.album),
                    duration: Some(time(snapshot.duration)),
                    cover_url: None,
                })
                .map_err(|error| error.to_string())?;
        }
        let progress = Some(MediaPosition(time(snapshot.position)));
        controls
            .set_playback(match snapshot.state.as_str() {
                "playing" => MediaPlayback::Playing { progress },
                "paused" => MediaPlayback::Paused { progress },
                _ => MediaPlayback::Stopped,
            })
            .map_err(|error| error.to_string())?;
    }
    if changed {
        guard.metadata = Some(metadata);
    }
    Ok(())
}
