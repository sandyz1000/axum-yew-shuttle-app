use axum::headers::authorization::Credentials;
use jwt_simple::prelude::*;

use crate::{api::UserId, error::AppResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct CustomClaim {
    pub user_id: UserId,
}

pub fn generate_jwt(user_id: UserId, key: &RS384KeyPair) -> AppResult<String> {
    let claims = Claims::with_custom_claims(CustomClaim { user_id }, Duration::from_days(30));
    Ok(key.sign(claims)?)
}

pub fn verify_jwt(token: &str, key: &RS384PublicKey) -> AppResult<CustomClaim> {
    let claims = key.verify_token(token, None)?;
    Ok(claims.custom)
}

pub struct JWTToken(pub String);

impl Credentials for JWTToken {
    const SCHEME: &'static str = "Token";

    fn decode(value: &axum::http::HeaderValue) -> Option<Self> {
        let mut it = value.to_str().ok()?.split_whitespace();
        let scheme = it.next()?;
        let token = it.next()?;

        if scheme != Self::SCHEME || it.next().is_some() {
            None?
        }

        Some(Self(token.to_string()))
    }

    fn encode(&self) -> axum::http::HeaderValue {
        unreachable!()
    }
}
