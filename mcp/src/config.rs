use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use url::Url;

const MAX_CONFIG_BYTES: u64 = 256 * 1024;
const MAX_HTTP_BYTES: usize = 1024 * 1024;
const MAX_EXECUTION_SECONDS: u64 = 360;
const MAX_CONCURRENCY: usize = 256;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    bind: String,
    resource: String,
    authorization_servers: Vec<String>,
    issuer: String,
    audience: String,
    required_scope: String,
    jwks_path: PathBuf,
    daemon_socket_path: PathBuf,
    tls_certificate_path: PathBuf,
    tls_private_key_path: PathBuf,
    allowed_hosts: Vec<String>,
    allowed_origins: Vec<String>,
    max_request_bytes: usize,
    max_response_bytes: usize,
    max_concurrency: usize,
    execution_timeout_seconds: u64,
}

#[derive(Clone)]
pub struct McpConfig {
    pub bind: SocketAddr,
    pub resource: String,
    pub mcp_path: String,
    pub metadata_path: String,
    pub metadata_url: String,
    pub authorization_servers: Vec<String>,
    pub issuer: String,
    pub audience: String,
    pub required_scope: String,
    pub jwks_path: PathBuf,
    pub daemon_socket_path: PathBuf,
    pub tls_certificate_path: PathBuf,
    pub tls_private_key_path: PathBuf,
    pub allowed_hosts: Vec<String>,
    pub allowed_origins: Vec<String>,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_concurrency: usize,
    pub execution_timeout: Duration,
}

pub fn load_config(path: impl AsRef<Path>) -> Result<McpConfig, String> {
    let path = path.as_ref();
    let bytes = read_bounded(path)?;
    let file: FileConfig = serde_json::from_slice(&bytes)
        .map_err(|error| format!("MCP config is invalid JSON: {error}"))?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    validate(file, base)
}

fn validate(file: FileConfig, base: &Path) -> Result<McpConfig, String> {
    let resource = secure_url("resource", &file.resource)?;
    if resource.query().is_some() || resource.fragment().is_some() {
        return Err("resource must not contain a query or fragment".into());
    }
    if resource.path() == "/" || resource.path().ends_with('/') {
        return Err("resource must identify the MCP endpoint path".into());
    }
    if file.audience != file.resource {
        return Err("audience must equal the protected resource URI".into());
    }
    let authorization_servers = validate_authorization_servers(&file)?;
    validate_scope(&file.required_scope)?;
    validate_hosts(&file.allowed_hosts)?;
    validate_origins(&file.allowed_origins)?;
    validate_limits(&file)?;
    let bind = file
        .bind
        .parse::<SocketAddr>()
        .map_err(|_| "bind must be an IP socket address".to_string())?;
    let metadata_path = format!("/.well-known/oauth-protected-resource{}", resource.path());
    let metadata_url = format!(
        "{}{}",
        resource.origin().ascii_serialization(),
        metadata_path
    );
    Ok(McpConfig {
        bind,
        resource: file.resource,
        mcp_path: resource.path().to_string(),
        metadata_path,
        metadata_url,
        authorization_servers,
        issuer: file.issuer,
        audience: file.audience,
        required_scope: file.required_scope,
        jwks_path: resolve(base, file.jwks_path),
        daemon_socket_path: resolve(base, file.daemon_socket_path),
        tls_certificate_path: resolve(base, file.tls_certificate_path),
        tls_private_key_path: resolve(base, file.tls_private_key_path),
        allowed_hosts: file.allowed_hosts,
        allowed_origins: file.allowed_origins,
        max_request_bytes: file.max_request_bytes,
        max_response_bytes: file.max_response_bytes,
        max_concurrency: file.max_concurrency,
        execution_timeout: Duration::from_secs(file.execution_timeout_seconds),
    })
}

fn validate_authorization_servers(file: &FileConfig) -> Result<Vec<String>, String> {
    if file.authorization_servers.is_empty() || file.authorization_servers.len() > 8 {
        return Err("authorization_servers must contain between 1 and 8 issuers".into());
    }
    for server in &file.authorization_servers {
        secure_url("authorization server", server)?;
    }
    secure_url("issuer", &file.issuer)?;
    if !file.authorization_servers.contains(&file.issuer) {
        return Err("issuer must appear in authorization_servers".into());
    }
    Ok(file.authorization_servers.clone())
}

fn secure_url(name: &str, value: &str) -> Result<Url, String> {
    let parsed = Url::parse(value).map_err(|_| format!("{name} must be an absolute URL"))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(format!("{name} must be a credential-free HTTPS URL"));
    }
    Ok(parsed)
}

fn validate_scope(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return Err("required_scope must be one printable OAuth scope token".into());
    }
    Ok(())
}

fn validate_hosts(values: &[String]) -> Result<(), String> {
    if values.is_empty()
        || values.len() > 32
        || values.iter().any(|value| {
            value.is_empty()
                || value == "*"
                || value.len() > 255
                || value.contains("//")
                || value.contains('/')
                || value.bytes().any(|byte| byte.is_ascii_whitespace())
        })
    {
        return Err("allowed_hosts must contain explicit host authorities".into());
    }
    Ok(())
}

fn validate_origins(values: &[String]) -> Result<(), String> {
    if values.is_empty() || values.len() > 32 {
        return Err("allowed_origins must contain between 1 and 32 HTTPS origins".into());
    }
    for value in values {
        let parsed = secure_url("allowed origin", value)?;
        if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
            return Err("allowed origins must not contain paths, queries, or fragments".into());
        }
    }
    Ok(())
}

fn validate_limits(file: &FileConfig) -> Result<(), String> {
    if !(1..=MAX_HTTP_BYTES).contains(&file.max_request_bytes)
        || !(1..=MAX_HTTP_BYTES).contains(&file.max_response_bytes)
        || !(1..=MAX_CONCURRENCY).contains(&file.max_concurrency)
        || !(1..=MAX_EXECUTION_SECONDS).contains(&file.execution_timeout_seconds)
    {
        return Err("MCP request, response, concurrency, or execution limit is invalid".into());
    }
    Ok(())
}

fn resolve(base: &Path, value: PathBuf) -> PathBuf {
    if value.is_absolute() {
        value
    } else {
        base.join(value)
    }
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;

    let file =
        fs::File::open(path).map_err(|error| format!("MCP config could not be read: {error}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("MCP config could not be read: {error}"))?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err("MCP config exceeds the byte limit".into());
    }
    Ok(bytes)
}
