use std::rc::Rc;

use gloo_net::http::Request;
use gloo_storage::{LocalStorage, Storage};
use yew::prelude::*;
use yew_hooks::{use_async_with_options, UseAsyncOptions};

use crate::api::{ApiError, UserAuth, UserAuthResp};

pub type AuthContext = UseReducerHandle<Auth>;

#[derive(PartialEq)]
pub enum Auth {
    Loading,
    Authorized(UserAuth),
    Unauthorized,
}

impl Reducible for Auth {
    type Action = Auth;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match &action {
            Auth::Authorized(user) => {
                LocalStorage::set("jwt", &user.token).unwrap();
            }
            Auth::Unauthorized => {
                LocalStorage::delete("jwt");
            }
            _ => {}
        }

        Rc::new(action)
    }
}

impl Auth {
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    pub fn is_authorized(&self) -> bool {
        matches!(self, Self::Authorized(_))
    }

    pub fn is_unauthorized(&self) -> bool {
        matches!(self, Self::Unauthorized)
    }

    pub fn user(&self) -> Option<&UserAuth> {
        match self {
            Self::Authorized(user) => Some(user),
            _ => None,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, thiserror::Error)]
pub enum UserAuthError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("{0}")]
    ApiError(#[from] Rc<ApiError>),
}

impl PartialEq for UserAuthError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unauthorized, Self::Unauthorized) => true,
            (Self::ApiError(p), Self::ApiError(q)) => Rc::ptr_eq(p, q),
            _ => false,
        }
    }
}

async fn get_user_auth(token: &str) -> Result<UserAuth, ApiError> {
    let resp = Request::get("/api/user")
        .header("Authorization", &format!("Token {token}"))
        .send()
        .await?
        .json::<UserAuthResp>()
        .await?;
    Ok(resp.user)
}

#[derive(PartialEq, Properties)]
pub struct AuthProviderProps {
    pub children: Children,
}

#[function_component]
pub fn AuthProvider(props: &AuthProviderProps) -> Html {
    let auth = use_reducer(|| Auth::Loading);

    use_async_with_options(
        {
            let auth = auth.clone();
            async move {
                if let Some(token) = LocalStorage::get::<String>("jwt").ok() {
                    if let Ok(user) = get_user_auth(&token).await {
                        auth.dispatch(Auth::Authorized(user));
                        return Ok(());
                    }
                }

                auth.dispatch(Auth::Unauthorized);
                Ok::<_, ()>(())
            }
        },
        UseAsyncOptions::enable_auto(),
    );

    html! {
        <ContextProvider<AuthContext> context={auth}>
            { for props.children.iter() }
        </ContextProvider<AuthContext>>
    }
}
