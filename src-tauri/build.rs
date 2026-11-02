fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "connection_settings",
            "connect_server",
            "publish_media",
        ]),
    ))
    .expect("failed to build desktop assets");
}
