mod api;
mod auth;
mod error;

use std::path::PathBuf;

use api::prepare_db;
use axum::{
    http::StatusCode,
    routing::{delete, get, get_service, post, put},
    Router,
};
use axum_extra::routing::SpaRouter;
use axum_macros::FromRef;
use jwt_simple::prelude::*;
use shuttle_secrets::SecretStore;
use shuttle_service::error::CustomError;
use sqlx::PgPool;
use sync_wrapper::SyncWrapper;
use tower_http::services::ServeDir;

#[derive(Clone, FromRef)]
struct AppState {
    pool: PgPool,
    key_pair: RS384KeyPair,
    public_key: RS384PublicKey,
}

#[shuttle_service::main]
async fn axum(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
    #[shuttle_static_folder::StaticFolder(folder = "assets/images")] images_folder: PathBuf,
    #[shuttle_static_folder::StaticFolder(folder = "dist")] dist_folder: PathBuf,
    #[shuttle_aws_rds::Postgres] pool: PgPool,
) -> shuttle_service::ShuttleAxum {
    let private_key = secret_store.get("private_key").unwrap();
    let public_key = secret_store.get("public_key").unwrap();

    let key_pair = RS384KeyPair::from_pem(&private_key)?;
    let public_key = RS384PublicKey::from_pem(&public_key)?;

    prepare_db(&pool).await.map_err(CustomError::new)?;

    let router = Router::new()
        .route("/api/users/login", post(api::login))
        .route("/api/users", post(api::registration))
        .route("/api/user", get(api::get_current_user))
        .route("/api/user", put(api::update_user))
        .route("/api/profiles/:username", get(api::get_profile))
        .route("/api/profiles/:username/follow", post(api::follow_user))
        .route("/api/profiles/:username/follow", delete(api::unfollow_user))
        .route("/api/articles", get(api::list_articles))
        .route("/api/articles/feed", get(api::feed_articles))
        .route("/api/articles/:slug", get(api::get_article))
        .route("/api/articles", post(api::create_article))
        .route("/api/articles/:slug", put(api::update_article))
        .route("/api/articles/:slug", delete(api::delete_article))
        .route("/api/articles/:slug/comments", post(api::add_comment))
        .route("/api/articles/:slug/comments", get(api::get_comments))
        .route(
            "/api/articles/:slug/comments/:id",
            delete(api::delete_comment),
        )
        .route("/api/articles/:slug/favorite", post(api::favorite_article))
        .route(
            "/api/articles/:slug/favorite",
            delete(api::unfavorite_article),
        )
        .route("/api/tags", get(api::get_tags))
        .route("/api/initialize", post(api::initialize))
        .merge(SpaRouter::new("/", dist_folder))
        .nest_service(
            "/images",
            get_service(ServeDir::new(images_folder)).handle_error(|err| async move {
                (StatusCode::NOT_FOUND, format!("Not Found: {err}"))
            }),
        )
        .with_state(AppState {
            pool,
            key_pair,
            public_key,
        });

    Ok(SyncWrapper::new(router))
}
