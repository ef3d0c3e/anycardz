//! Basic auth: username + email + password, session cookie. No email
//! verification yet (AC-35).

use std::future::Future;
use std::pin::Pin;

use actix_web::cookie::{Cookie, SameSite, time::Duration as CookieDuration};
use actix_web::{FromRequest, HttpRequest, HttpResponse, dev::Payload, post, web};
use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use chrono::{Duration, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::AppState;
use crate::error::{ApiError, ApiResult};
use crate::game::STARTING_COINS;
use crate::schema::{sessions, users};

pub const SESSION_COOKIE: &str = "session";
const SESSION_DAYS: i64 = 30;

diesel::define_sql_function!(fn lower(x: diesel::sql_types::Text) -> diesel::sql_types::Text);

/// The logged-in user, extracted from the session cookie. Handlers taking it
/// answer 401 to anonymous requests.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = users)]
pub struct CurrentUser {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub coins: i64,
}

fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

impl FromRequest for CurrentUser {
    type Error = ApiError;
    type Future = Pin<Box<dyn Future<Output = Result<CurrentUser, ApiError>>>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let state = req.app_data::<web::Data<AppState>>().cloned();
        let token = req.cookie(SESSION_COOKIE).map(|c| c.value().to_owned());
        Box::pin(async move {
            let (Some(state), Some(token)) = (state, token) else {
                return Err(ApiError::Unauthorized);
            };
            let mut conn = state.pool.get().await?;
            sessions::table
                .inner_join(users::table)
                .filter(sessions::token_hash.eq(token_hash(&token)))
                .filter(sessions::expires_at.gt(Utc::now()))
                .select(CurrentUser::as_select())
                .first(&mut conn)
                .await
                .optional()?
                .ok_or(ApiError::Unauthorized)
        })
    }
}

#[derive(Deserialize)]
pub struct RegisterBody {
    username: String,
    email: String,
    password: String,
}

#[derive(Deserialize)]
pub struct LoginBody {
    /// Username or email
    login: String,
    password: String,
}

pub fn validate_username(name: &str) -> ApiResult<()> {
    let ok_len = (3..=20).contains(&name.chars().count());
    let ok_chars = name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if ok_len && ok_chars { Ok(()) } else { Err(ApiError::Invalid("invalid_username")) }
}

pub fn validate_email(email: &str) -> ApiResult<()> {
    let ok = email.len() <= 254
        && email.split_once('@').is_some_and(|(user, host)| {
            !user.is_empty() && host.contains('.') && !host.starts_with('.') && !host.ends_with('.')
        })
        && !email.contains(char::is_whitespace);
    if ok { Ok(()) } else { Err(ApiError::Invalid("invalid_email")) }
}

pub fn validate_password(password: &str) -> ApiResult<()> {
    if (8..=128).contains(&password.chars().count()) { Ok(()) } else { Err(ApiError::Invalid("weak_password")) }
}

fn hash_password(password: &str) -> ApiResult<String> {
    let mut raw = [0u8; 16];
    rand::rng().fill_bytes(&mut raw);
    let salt = SaltString::encode_b64(&raw).map_err(|e| ApiError::Internal(anyhow::anyhow!("salt: {e}")))?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("hashing: {e}")))
}

/// Opens a session and returns the response carrying its cookie
async fn start_session(state: &AppState, user_id: i64, status: actix_web::http::StatusCode) -> ApiResult<HttpResponse> {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let token = hex::encode(bytes);
    let mut conn = state.pool.get().await?;
    diesel::insert_into(sessions::table)
        .values((
            sessions::user_id.eq(user_id),
            sessions::token_hash.eq(token_hash(&token)),
            sessions::expires_at.eq(Utc::now() + Duration::days(SESSION_DAYS)),
        ))
        .execute(&mut conn)
        .await?;
    let cookie = Cookie::build(SESSION_COOKIE, token)
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.config.secure_cookies)
        .max_age(CookieDuration::days(SESSION_DAYS))
        .finish();
    Ok(HttpResponse::build(status).cookie(cookie).json(serde_json::json!({ "ok": true })))
}

#[post("/auth/register")]
pub async fn register(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<RegisterBody>,
) -> ApiResult<HttpResponse> {
    let RegisterBody { username, email, password } = body.into_inner();
    let (username, email) = (username.trim().to_owned(), email.trim().to_owned());
    // Every sign-up comes with coins: a few per address, then one every 10 minutes
    let ip = crate::ratelimit::client_ip(&req, &state.config.trusted_proxies);
    if let Some(wait) = state.limits.register_ip.wait(&ip) {
        state.metrics.auth("register", "limited");
        return Err(ApiError::RetryAfter(wait));
    }
    validate_username(&username)?;
    validate_email(&email)?;
    validate_password(&password)?;
    state.limits.register_ip.hit(&ip);
    // Argon2 is slow on purpose: keep it off the async workers
    let hash = web::block(move || hash_password(&password)).await.map_err(|e| ApiError::Internal(e.into()))??;

    let mut conn = state.pool.get().await?;
    let inserted = diesel::insert_into(users::table)
        .values((
            users::username.eq(&username),
            users::email.eq(&email),
            users::password_hash.eq(hash),
            users::coins.eq(STARTING_COINS),
        ))
        .returning(users::id)
        .get_result::<i64>(&mut conn)
        .await;
    let id = match inserted {
        Ok(id) => id,
        Err(diesel::result::Error::DatabaseError(diesel::result::DatabaseErrorKind::UniqueViolation, info)) => {
            let taken =
                if info.constraint_name() == Some("users_email_key") { "email_taken" } else { "username_taken" };
            return Err(ApiError::Conflict(taken));
        }
        Err(e) => return Err(e.into()),
    };
    drop(conn);
    state.metrics.auth("register", "ok");
    state.metrics.earned("signup", STARTING_COINS);
    start_session(&state, id, actix_web::http::StatusCode::CREATED).await
}

#[post("/auth/login")]
pub async fn login(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<LoginBody>,
) -> ApiResult<HttpResponse> {
    let LoginBody { login, password } = body.into_inner();
    let login = login.trim().to_lowercase();
    // Only failures cost: per account against guessing one password, per
    // address against sweeping many accounts
    let ip = crate::ratelimit::client_ip(&req, &state.config.trusted_proxies);
    let limits = &state.limits;
    let wait = [limits.login_account.wait(&login), limits.login_ip.wait(&ip)].into_iter().flatten().max();
    if let Some(wait) = wait {
        state.metrics.auth("login", "limited");
        return Err(ApiError::RetryAfter(wait));
    }
    let failed = || {
        limits.login_account.hit(&login);
        limits.login_ip.hit(&ip);
        state.metrics.auth("login", "failed");
        ApiError::Invalid("invalid_credentials")
    };
    let mut conn = state.pool.get().await?;
    let found: Option<(i64, String)> = users::table
        .filter(lower(users::username).eq(&login).or(lower(users::email).eq(&login)))
        .select((users::id, users::password_hash))
        .first(&mut conn)
        .await
        .optional()?;
    drop(conn);
    let Some((id, hash)) = found else {
        return Err(failed());
    };
    let ok = web::block(move || {
        PasswordHash::new(&hash)
            .is_ok_and(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
    })
    .await
    .map_err(|e| ApiError::Internal(e.into()))?;
    if !ok {
        return Err(failed());
    }
    state.metrics.auth("login", "ok");
    start_session(&state, id, actix_web::http::StatusCode::OK).await
}

#[post("/auth/logout")]
pub async fn logout(state: web::Data<AppState>, req: HttpRequest) -> ApiResult<HttpResponse> {
    if let Some(cookie) = req.cookie(SESSION_COOKIE) {
        let mut conn = state.pool.get().await?;
        diesel::delete(sessions::table.filter(sessions::token_hash.eq(token_hash(cookie.value()))))
            .execute(&mut conn)
            .await?;
    }
    let mut gone = Cookie::build(SESSION_COOKIE, "").path("/").finish();
    gone.make_removal();
    Ok(HttpResponse::Ok().cookie(gone).json(serde_json::json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_rules() {
        assert!(validate_username("teste").is_ok());
        assert!(validate_username("mira_k.42").is_ok());
        assert!(validate_username("ab").is_err());
        assert!(validate_username("has space").is_err());
        assert!(validate_username(&"a".repeat(21)).is_err());
    }

    #[test]
    fn email_rules() {
        assert!(validate_email("a@b.co").is_ok());
        assert!(validate_email("nope").is_err());
        assert!(validate_email("a@b").is_err());
        assert!(validate_email("a @b.co").is_err());
    }

    #[test]
    fn password_round_trip() {
        let hash = hash_password("123456789").unwrap();
        let parsed = PasswordHash::new(&hash).unwrap();
        assert!(Argon2::default().verify_password(b"123456789", &parsed).is_ok());
        assert!(Argon2::default().verify_password(b"wrong-pass", &parsed).is_err());
        assert!(validate_password("short").is_err());
    }
}
