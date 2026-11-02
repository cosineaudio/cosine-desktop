use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use url::Url;

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub server: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub maximized: bool,
}

pub fn server_url(address: &str) -> Result<Url, String> {
    let url = Url::parse(address.trim())
        .map_err(|_| "Enter a full server address, including https:// or http://.")?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("The server address must use https:// or http://.".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(
            "Enter the server address without credentials. Sign in on cosine's login screen."
                .into(),
        );
    }
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err("Enter only the server origin (for example https://music.example.com), without a page path.".into());
    }
    Ok(url)
}

pub fn read(path: &Path) -> Result<Settings, String> {
    match fs::read(path) {
        Ok(data) => serde_json::from_slice(&data).map_err(|_| "The desktop settings file is damaged. Rename settings.json in the app's configuration directory to reset it.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(error) => Err(format!("Cannot read desktop settings: {error}")),
    }
}

pub fn write(path: &Path, settings: &Settings) -> Result<(), String> {
    let parent = path.parent().ok_or("Invalid settings path")?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Cannot create settings directory: {error}"))?;
    let temp = path.with_extension("tmp");
    let bytes = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(&temp, bytes).map_err(|error| format!("Cannot save desktop settings: {error}"))?;
    fs::rename(temp, path).map_err(|error| format!("Cannot save desktop settings: {error}"))
}

pub async fn probe(client: &reqwest::Client, server: &Url) -> Result<(), String> {
    let mut response = client.get(server.join("auth/config").map_err(|error| error.to_string())?)
        .send().await.map_err(|error| -> String {
            if error.is_timeout() { "The server did not respond within 10 seconds. Check the address and your connection.".into() }
            else if error.is_connect() { "Cannot connect to this server. Check the address, TLS certificate, and whether cosine is running.".into() }
            else { "The server connection failed. Check the address and your connection.".into() }
        })?;
    if !response.status().is_success() {
        return Err(format!(
            "The server returned HTTP {} for /auth/config.",
            response.status().as_u16()
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "The connection was interrupted while checking the server.")?
    {
        if bytes.len() + chunk.len() > 65536 {
            return Err(
                "This address does not appear to be a cosine server (/auth/config is too large)."
                    .into(),
            );
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
        "This address does not appear to be a cosine server (/auth/config did not return JSON)."
    })?;
    if !value["localLoginEnabled"].is_boolean() || !value["oidcEnabled"].is_boolean() {
        return Err(
            "The server's authentication configuration is not compatible with cosine.".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_origins_and_normalizes_them() {
        assert_eq!(
            server_url(" https://MUSIC.example.com ").unwrap().as_str(),
            "https://music.example.com/"
        );
        assert_eq!(server_url("http://[::1]:8080/").unwrap().port(), Some(8080));
    }
    #[test]
    fn rejects_credentials_paths_and_non_web_schemes() {
        for address in [
            "music.local",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://a:b@music.local",
            "https://music.local/albums",
            "https://music.local/?token=secret",
            "https://music.local/#album",
        ] {
            assert!(server_url(address).is_err(), "{address}");
        }
    }
    #[test]
    fn missing_settings_are_defaults_and_corruption_is_reported() {
        let path =
            std::env::temp_dir().join(format!("cosine-settings-test-{}.json", std::process::id()));
        assert!(read(&path).unwrap().server.is_none());
        write(
            &path,
            &Settings {
                server: Some("https://music.local/".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            read(&path).unwrap().server.as_deref(),
            Some("https://music.local/")
        );
        fs::write(&path, "bad json").unwrap();
        assert!(read(&path).is_err());
        fs::remove_file(path).unwrap();
    }

    fn probe_response(status: &str, body: &str) -> Result<(), String> {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let server = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let response = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}", body.len());
        let task = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0; 4096];
            let length = stream.read(&mut buffer).unwrap();
            assert!(String::from_utf8_lossy(&buffer[..length]).starts_with("GET /auth/config "));
            let _ = stream.write_all(response.as_bytes());
        });
        let result = tauri::async_runtime::block_on(async {
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap();
            probe(&client, &server).await
        });
        task.join().unwrap();
        result
    }

    #[test]
    fn checks_cosine_configuration_before_navigating() {
        assert!(probe_response(
            "200 OK",
            r#"{"localLoginEnabled":true,"oidcEnabled":false}"#
        )
        .is_ok());
        assert!(probe_response(
            "200 OK",
            r#"{"localLoginEnabled":false,"oidcEnabled":true}"#
        )
        .is_ok());
        assert!(probe_response("200 OK", "<html>Another app</html>").is_err());
        assert!(probe_response("200 OK", r#"{"localLoginEnabled":"true"}"#).is_err());
        assert!(probe_response("404 Not Found", "{}")
            .unwrap_err()
            .contains("404"));
        assert!(probe_response("302 Found", "{}")
            .unwrap_err()
            .contains("302"));
        assert!(probe_response("200 OK", &"a".repeat(65537))
            .unwrap_err()
            .contains("too large"));
    }
}
