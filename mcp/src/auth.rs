use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::jwk::{AlgorithmParameters, Jwk, JwkSet, KeyAlgorithm};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;

const MAX_JWKS_BYTES: u64 = 1024 * 1024;
const CLOCK_SKEW_SECONDS: u64 = 30;

pub trait Clock: Send + Sync {
    fn now_unix(&self) -> u64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

#[derive(Clone)]
pub struct AuthValidator {
    keys: Arc<JwkSet>,
    issuer: Arc<str>,
    audience: Arc<str>,
    required_scope: Arc<str>,
    clock: Arc<dyn Clock>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthFailure {
    InvalidToken,
    InsufficientScope,
}

#[derive(Deserialize)]
struct AccessClaims {
    exp: u64,
    #[serde(default)]
    nbf: Option<u64>,
    scope: String,
}

impl AuthValidator {
    pub fn from_jwks_file(
        path: impl AsRef<Path>,
        issuer: impl Into<Arc<str>>,
        audience: impl Into<Arc<str>>,
        required_scope: impl Into<Arc<str>>,
    ) -> Result<Self, String> {
        let keys = load_jwks(path.as_ref())?;
        Self::new(
            keys,
            issuer,
            audience,
            required_scope,
            Arc::new(SystemClock),
        )
    }

    pub fn new(
        keys: JwkSet,
        issuer: impl Into<Arc<str>>,
        audience: impl Into<Arc<str>>,
        required_scope: impl Into<Arc<str>>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, String> {
        validate_jwks(&keys)?;
        Ok(Self {
            keys: Arc::new(keys),
            issuer: issuer.into(),
            audience: audience.into(),
            required_scope: required_scope.into(),
            clock,
        })
    }

    pub fn validate(&self, token: &str) -> Result<(), AuthFailure> {
        if token.is_empty() || token.len() > 16 * 1024 {
            return Err(AuthFailure::InvalidToken);
        }
        let header = decode_header(token).map_err(|_| AuthFailure::InvalidToken)?;
        let kid = header.kid.as_deref().ok_or(AuthFailure::InvalidToken)?;
        let jwk = self.keys.find(kid).ok_or(AuthFailure::InvalidToken)?;
        let algorithm = jwk_algorithm(jwk).ok_or(AuthFailure::InvalidToken)?;
        if header.alg != algorithm {
            return Err(AuthFailure::InvalidToken);
        }
        let key = DecodingKey::from_jwk(jwk).map_err(|_| AuthFailure::InvalidToken)?;
        let mut validation = Validation::new(algorithm);
        validation.set_issuer(&[self.issuer.as_ref()]);
        validation.set_audience(&[self.audience.as_ref()]);
        validation.set_required_spec_claims(&["exp", "iss", "aud"]);
        validation.validate_exp = false;
        validation.validate_nbf = false;
        let claims = decode::<AccessClaims>(token, &key, &validation)
            .map_err(|_| AuthFailure::InvalidToken)?
            .claims;
        let now = self.clock.now_unix();
        if claims.exp.saturating_add(CLOCK_SKEW_SECONDS) < now
            || claims
                .nbf
                .is_some_and(|nbf| nbf > now.saturating_add(CLOCK_SKEW_SECONDS))
        {
            return Err(AuthFailure::InvalidToken);
        }
        if !claims
            .scope
            .split_ascii_whitespace()
            .any(|scope| scope == self.required_scope.as_ref())
        {
            return Err(AuthFailure::InsufficientScope);
        }
        Ok(())
    }
}

fn load_jwks(path: &Path) -> Result<JwkSet, String> {
    use std::io::Read;

    let file = fs::File::open(path).map_err(|error| format!("JWKS could not be read: {error}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_JWKS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("JWKS could not be read: {error}"))?;
    if bytes.len() as u64 > MAX_JWKS_BYTES {
        return Err("JWKS exceeds the byte limit".into());
    }
    serde_json::from_slice(&bytes).map_err(|error| format!("JWKS is invalid JSON: {error}"))
}

fn validate_jwks(keys: &JwkSet) -> Result<(), String> {
    if keys.keys.is_empty() || keys.keys.len() > 64 {
        return Err("JWKS must contain between 1 and 64 keys".into());
    }
    let mut ids = BTreeSet::new();
    for key in &keys.keys {
        let id = key
            .common
            .key_id
            .as_deref()
            .filter(|value| !value.is_empty() && value.len() <= 256)
            .ok_or_else(|| "every JWK must have a bounded kid".to_string())?;
        if !ids.insert(id) || jwk_algorithm(key).is_none() {
            return Err("JWKS contains a duplicate kid or unsupported signing key".into());
        }
        if matches!(key.algorithm, AlgorithmParameters::OctetKey(_)) {
            return Err("JWKS must not contain symmetric signing secrets".into());
        }
    }
    Ok(())
}

fn jwk_algorithm(key: &Jwk) -> Option<Algorithm> {
    match key.common.key_algorithm? {
        KeyAlgorithm::RS256 => Some(Algorithm::RS256),
        KeyAlgorithm::RS384 => Some(Algorithm::RS384),
        KeyAlgorithm::RS512 => Some(Algorithm::RS512),
        KeyAlgorithm::PS256 => Some(Algorithm::PS256),
        KeyAlgorithm::PS384 => Some(Algorithm::PS384),
        KeyAlgorithm::PS512 => Some(Algorithm::PS512),
        KeyAlgorithm::ES256 => Some(Algorithm::ES256),
        KeyAlgorithm::ES384 => Some(Algorithm::ES384),
        KeyAlgorithm::EdDSA => Some(Algorithm::EdDSA),
        _ => None,
    }
}
