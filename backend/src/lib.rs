mod api;
mod auth;
mod error;

use std::path::PathBuf;

use api::prepare_db;
use axum::{
    extract::FromRef,
    http::StatusCode,
    routing::{delete, get, get_service, post, put},
    Router,
};
use axum_extra::routing::SpaRouter;
use jsonwebtoken::{DecodingKey, EncodingKey};
use shuttle_secrets::SecretStore;
use shuttle_service::error::CustomError;
use sqlx::PgPool;
use sync_wrapper::SyncWrapper;
use tower_http::{compression::CompressionLayer, services::ServeDir};

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl FromRef<AppState> for PgPool {
    fn from_ref(app_state: &AppState) -> PgPool {
        app_state.pool.clone()
    }
}

impl FromRef<AppState> for EncodingKey {
    fn from_ref(app_state: &AppState) -> EncodingKey {
        app_state.encoding_key.clone()
    }
}

impl FromRef<AppState> for DecodingKey {
    fn from_ref(app_state: &AppState) -> DecodingKey {
        app_state.decoding_key.clone()
    }
}

#[shuttle_service::main]
async fn axum(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
    #[shuttle_static_folder::StaticFolder(folder = "images")] images_folder: PathBuf,
    #[shuttle_static_folder::StaticFolder(folder = "dist")] dist_folder: PathBuf,
    #[shuttle_aws_rds::Postgres] pool: PgPool,
) -> shuttle_service::ShuttleAxum {
    log::info!("xxx: 1");
    let private_key = secret_store.get("private_key").unwrap();
    log::info!("xxx: 2");
    let public_key = secret_store.get("public_key").unwrap();
    log::info!("xxx: 3");

    let encoding_key = EncodingKey::from_rsa_pem(private_key.as_bytes()).unwrap();
    log::info!("xxx: 4");
    let decoding_key = DecodingKey::from_rsa_pem(public_key.as_bytes()).unwrap();
    log::info!("xxx: 5");

    prepare_db(&pool).await.map_err(CustomError::new)?;
    log::info!("xxx: 6");

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
        .merge(SpaRouter::new("/", dist_folder).index_file("index.html"))
        .nest_service(
            "/images",
            get_service(ServeDir::new(images_folder)).handle_error(|err| async move {
                (StatusCode::NOT_FOUND, format!("Not Found: {err}"))
            }),
        )
        .with_state(AppState {
            pool,
            encoding_key,
            decoding_key,
        })
        .layer(CompressionLayer::new());

    Ok(SyncWrapper::new(router))
}
