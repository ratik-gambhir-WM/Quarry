use super::{model::*, repository::IdentityRepository};
use crate::shared::error::{ServiceError, ServiceResult};
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, Semaphore};
const SESSION_SECONDS: i64 = 8 * 60 * 60;
#[derive(Clone)]
pub struct IdentityService {
    repository: IdentityRepository,
    workers: Arc<Semaphore>,
    limits: Arc<Mutex<HashMap<String, (Instant, usize)>>>,
    revoked: broadcast::Sender<String>,
}
impl IdentityService {
    pub fn new(repository: IdentityRepository) -> Self {
        let (revoked, _) = broadcast::channel(256);
        Self {
            repository,
            workers: Arc::new(Semaphore::new(4)),
            limits: Arc::new(Mutex::new(HashMap::new())),
            revoked,
        }
    }
    pub fn revocations(&self) -> broadcast::Receiver<String> {
        self.revoked.subscribe()
    }
    pub fn admit(&self, key: String, max: usize) -> ServiceResult<()> {
        let mut limits = self
            .limits
            .lock()
            .map_err(|_| ServiceError::internal("auth limiter unavailable"))?;
        limits.retain(|_, (since, _)| since.elapsed() < Duration::from_secs(60));
        if limits.len() >= 10_000 && !limits.contains_key(&key) {
            return Err(ServiceError::RateLimited);
        }
        let entry = limits.entry(key).or_insert((Instant::now(), 0));
        if entry.1 >= max {
            return Err(ServiceError::RateLimited);
        }
        entry.1 += 1;
        Ok(())
    }
    pub async fn register(&self, mut input: RegisterInput) -> ServiceResult<SessionResponse> {
        let password = std::mem::take(&mut input.password);
        let hash = self
            .password(password, None)
            .await?
            .ok_or_else(|| ServiceError::internal("password hashing failed"))?;
        let (token, digest) = token()?;
        let now = now();
        let expires = now + SESSION_SECONDS;
        let user = self
            .repository
            .register(input, hash, digest, now, expires)
            .await?;
        Ok(SessionResponse {
            user,
            access_token: token,
            token_type: "Bearer",
            expires_at: timestamp(expires),
        })
    }
    pub async fn login(&self, input: LoginInput) -> ServiceResult<SessionResponse> {
        self.admit(format!("username:{}", input.username), 5)?;
        let record = self.repository.credentials(input.username.clone()).await?;
        // A valid fixed PHC record keeps unknown-user verification on the same expensive path.
        let dummy = "$argon2id$v=19$m=19456,t=2,p=1$cXVhcnJ5LWR1bW15LXNhbHQ$K4JfFRrPKf2cJ5a0WuD7Xlf8eMR6KbGCj5VLca0QesE";
        let hash = record
            .as_ref()
            .map(|(_, hash)| hash.as_str())
            .unwrap_or(dummy)
            .to_string();
        let verified = self.password(input.password, Some(hash)).await?.is_some();
        let id = record
            .filter(|_| verified)
            .map(|(id, _)| id)
            .ok_or(ServiceError::Unauthorized)?;
        if let Ok(mut limits) = self.limits.lock() {
            limits.remove(&format!("username:{}", input.username));
        }
        let (token, digest) = token()?;
        let now = now();
        let expires = now + SESSION_SECONDS;
        self.repository
            .create_session(id, digest, now, expires)
            .await?;
        Ok(SessionResponse {
            user: self.repository.user(id).await?,
            access_token: token,
            token_type: "Bearer",
            expires_at: timestamp(expires),
        })
    }
    async fn password(
        &self,
        password: String,
        hash: Option<String>,
    ) -> ServiceResult<Option<String>> {
        let permit = self
            .workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| ServiceError::RateLimited)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let argon = Argon2::default();
            if let Some(hash) = hash {
                let parsed = PasswordHash::new(&hash)
                    .map_err(|_| ServiceError::internal("invalid stored password hash"))?;
                return Ok(argon
                    .verify_password(password.as_bytes(), &parsed)
                    .ok()
                    .map(|_| String::new()));
            }
            let mut salt = [0u8; 16];
            getrandom::fill(&mut salt)
                .map_err(|_| ServiceError::internal("secure randomness unavailable"))?;
            let salt = SaltString::encode_b64(&salt)
                .map_err(|_| ServiceError::internal("salt encoding failed"))?;
            argon
                .hash_password(password.as_bytes(), &salt)
                .map(|hash| Some(hash.to_string()))
                .map_err(|_| ServiceError::internal("password hashing failed"))
        })
        .await
        .map_err(|_| ServiceError::internal("password worker failed"))?
    }
    pub async fn authenticate(&self, token: &str) -> ServiceResult<AuthenticatedUser> {
        if token.len() != 43
            || !token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(ServiceError::Unauthorized);
        }
        self.authenticate_hash(digest(token)).await
    }
    pub async fn authenticate_hash(&self, hash: String) -> ServiceResult<AuthenticatedUser> {
        let (id, expires_at) = self
            .repository
            .session(hash.clone())
            .await?
            .filter(|(_, expires)| *expires > now())
            .ok_or(ServiceError::Unauthorized)?;
        Ok(AuthenticatedUser {
            user: self.repository.user(id).await?,
            expires_at,
            token_hash: hash,
        })
    }
    pub async fn logout(&self, actor: &AuthenticatedUser) -> ServiceResult<()> {
        self.repository
            .revoke(actor.token_hash.clone(), now())
            .await?;
        let _ = self.revoked.send(actor.token_hash.clone());
        Ok(())
    }
}
pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
pub fn timestamp(seconds: i64) -> String {
    chrono::DateTime::from_timestamp(seconds, 0)
        .map(|d| d.to_rfc3339())
        .unwrap_or_default()
}
fn digest(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
fn token() -> ServiceResult<(String, String)> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|_| ServiceError::internal("secure randomness unavailable"))?;
    let token = URL_SAFE_NO_PAD.encode(bytes);
    let hash = digest(&token);
    Ok((token, hash))
}
