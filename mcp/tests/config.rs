use std::fs;

use serde_json::json;
use tempfile::TempDir;
use tts29_mcp::load_config;

#[test]
fn config_requires_https_and_refuses_embedded_credentials() {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("mcp.json");
    let mut value = valid_config();
    value["resource"] = json!("http://tts.example.test/mcp");
    value["audience"] = value["resource"].clone();
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(load_config(&path).err().unwrap().contains("HTTPS"));

    let mut value = valid_config();
    value["access_token"] = json!("must-not-live-in-config");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(load_config(&path).err().unwrap().contains("unknown field"));
}

#[test]
fn config_derives_rfc9728_path_metadata_from_the_resource() {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("mcp.json");
    fs::write(&path, serde_json::to_vec(&valid_config()).unwrap()).unwrap();

    let config = load_config(path).unwrap();

    assert_eq!(config.mcp_path, "/mcp");
    assert_eq!(
        config.metadata_url,
        "https://tts.example.test/.well-known/oauth-protected-resource/mcp"
    );
}

fn valid_config() -> serde_json::Value {
    json!({
        "bind": "127.0.0.1:8443",
        "resource": "https://tts.example.test/mcp",
        "authorization_servers": ["https://auth.example.test"],
        "issuer": "https://auth.example.test",
        "audience": "https://tts.example.test/mcp",
        "required_scope": "tts29:publish",
        "jwks_path": "authorization-server.jwks.json",
        "daemon_socket_path": "daemon.sock",
        "tls_certificate_path": "cert.pem",
        "tls_private_key_path": "key.pem",
        "allowed_hosts": ["tts.example.test"],
        "allowed_origins": ["https://assistant.example.test"],
        "max_request_bytes": 131072,
        "max_response_bytes": 65536,
        "max_concurrency": 16,
        "execution_timeout_seconds": 330
    })
}
