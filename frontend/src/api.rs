use std::{collections::HashMap, rc::Rc};

use chrono::{DateTime, Utc};
use gloo_net::http::Request;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use validator::ValidationError;

const DEFAULT_USER_IMAGE: &str = "/images/smiley-cyrus.jpeg";

#[derive(Deserialize)]
struct JsonError<T> {
    error: T,
}

#[derive(Debug, Deserialize, thiserror::Error)]
#[error("validation error: {0:?}")]
pub struct ValidationErrors(pub HashMap<String, Vec<ValidationError>>);

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("network error")]
    NetworkError(#[from] gloo_net::Error),
    #[error("{0}")]
    ValidationError(#[from] ValidationErrors),
    #[error("{0}")]
    AppError(serde_json::Value),
}

impl ApiError {
    pub fn to_vec_string(&self) -> Vec<String> {
        match self {
            ApiError::NetworkError(err) => vec![format!("network error: {}", err)],
            ApiError::ValidationError(err) => err
                .0
                .iter()
                .flat_map(|(_, message)| {
                    message
                        .iter()
                        .flat_map(|err| err.message.as_ref().map(|s| s.to_string()))
                })
                .collect(),
            ApiError::AppError(json) => {
                log::error!("{json:?}");

                json.as_object()
                    .unwrap()
                    .iter()
                    .map(|(key, value)| format!("{key} {}", value.as_str().unwrap()))
                    .collect()
            }
        }
    }
}

#[derive(PartialEq, Clone, Deserialize, Debug)]
pub struct UserAuth {
    pub username: String,
    pub email: String,
    pub token: String,
    pub bio: Option<String>,
    pub image: Option<String>,
}

impl UserAuth {
    pub fn image(&self) -> &str {
        let ret = self.image.as_deref().unwrap_or(DEFAULT_USER_IMAGE);
        if ret.is_empty() {
            DEFAULT_USER_IMAGE
        } else {
            ret
        }
    }
}

#[derive(Deserialize)]
pub struct UserAuthResp {
    pub user: UserAuth,
}

#[allow(dead_code)]
#[derive(PartialEq, Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Article {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub body: String,
    pub tag_list: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub favorited: bool,
    pub favorites_count: u32,
    pub author: UserProfile,
}

#[derive(Deserialize)]
pub struct ArticleResp {
    pub article: Article,
}

#[allow(dead_code)]
#[derive(PartialEq, Debug, Clone, Deserialize)]
pub struct UserProfile {
    pub username: String,
    pub bio: Option<String>,
    pub image: Option<String>,
    pub following: bool,
}

impl UserProfile {
    pub fn image(&self) -> &str {
        self.image.as_deref().unwrap_or(DEFAULT_USER_IMAGE)
    }
}

#[derive(Deserialize)]
pub struct UserProfileResp {
    pub profile: UserProfile,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultipleArticle {
    pub articles: Vec<Article>,
    pub articles_count: usize,
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub body: String,
    pub author: UserProfile,
}

#[derive(Deserialize)]
pub struct CommentsResp {
    pub comments: Vec<Comment>,
}

#[derive(Deserialize)]
pub struct CommentResp {
    pub comment: Comment,
}

#[derive(Deserialize)]
pub struct TagsResp {
    pub tags: Vec<String>,
}

pub struct ApiRequest(Request);

impl ApiRequest {
    pub fn get(url: impl AsRef<str>) -> Self {
        Self(Request::get(url.as_ref()))
    }

    pub fn post(url: impl AsRef<str>) -> Self {
        Self(Request::post(url.as_ref()))
    }

    pub fn put(url: impl AsRef<str>) -> Self {
        Self(Request::put(url.as_ref()))
    }

    pub fn delete(url: impl AsRef<str>) -> Self {
        Self(Request::delete(url.as_ref()))
    }

    pub fn query<'a, T, V>(self, params: T) -> Self
    where
        T: IntoIterator<Item = (&'a str, V)>,
        V: AsRef<str>,
    {
        Self(self.0.query(params))
    }

    pub fn auth(self, auth: Option<&UserAuth>) -> Self {
        if let Some(auth) = auth {
            Self(
                self.0
                    .header("Authorization", &format!("Token {}", auth.token)),
            )
        } else {
            self
        }
    }

    pub fn json(self, json: &impl Serialize) -> Self {
        Self(self.0.json(json).unwrap())
    }

    pub async fn json_response<T: DeserializeOwned>(self) -> Result<T, ApiError> {
        // log::info!("Request: {:?}", self.0);

        let resp = self.0.send().await.map_err(|err| {
            log::error!("Network error: {err:?}");
            ApiError::NetworkError(err)
        })?;

        // log::info!("Response: {resp:?}");

        if resp.ok() {
            Ok(resp.json().await.map_err(|err| {
                log::error!("Response json error: {err:?}");
                err
            })?)
        } else if resp.status() == 422 {
            let json: JsonError<ValidationErrors> = resp.json().await?;
            Err(ApiError::ValidationError(json.error))?
        } else {
            let json: JsonError<serde_json::Value> = resp.json().await?;
            Err(ApiError::AppError(json.error))?
        }
    }
}

pub async fn register_user(
    username: &str,
    email: &str,
    password: &str,
) -> Result<UserAuth, Rc<ApiError>> {
    // log::info!("Register user: username: {username:?} email: {email:?} password: {password:?}");

    let resp: UserAuthResp = ApiRequest::post("/api/users")
        .json(&json!({
            "user": {
                "username": username,
                "email": email,
                "password": password,
            }
        }))
        .json_response()
        .await?;

    Ok(resp.user)
}

pub async fn login_user(email: &str, password: &str) -> Result<UserAuth, Rc<ApiError>> {
    // log::info!("Login user: email: {email:?} password: {password:?}");

    let resp: UserAuthResp = ApiRequest::post("/api/users/login")
        .json(&json!({
            "user": {
                "email": email,
                "password": password,
            }
        }))
        .json_response()
        .await?;

    Ok(resp.user)
}
