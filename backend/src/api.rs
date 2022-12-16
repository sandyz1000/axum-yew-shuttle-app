use axum::{
    extract::{Path, Query, State},
    headers::Authorization,
    response::IntoResponse,
    Json, TypedHeader,
};
use chrono::{DateTime, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Executor, PgPool};
use validator::Validate;

use crate::{
    auth::{self, JWTToken},
    error::{AppError, AppResult},
};

pub async fn prepare_db(pool: &PgPool) -> Result<(), sqlx::Error> {
    pool.execute(include_str!("../schema.sql")).await?;
    Ok(())
}

pub async fn initialize_db(pool: &PgPool) -> Result<(), sqlx::Error> {
    pool.execute(include_str!("../down.sql")).await?;
    pool.execute(include_str!("../schema.sql")).await?;
    Ok(())
}

pub async fn initialize(State(pool): State<PgPool>) -> AppResult<impl IntoResponse> {
    initialize_db(&pool).await?;
    Ok(Json(json!({ "message": "ok" })))
}

pub type UserId = i32;

#[derive(Debug, Default, Serialize)]
struct UserAuth {
    #[serde(skip)]
    id: UserId,
    username: String,
    email: String,
    token: Option<String>,
    #[serde(skip)]
    hash: String,
    bio: Option<String>,
    image: Option<String>,
}

#[derive(Debug, Default, Serialize, sqlx::Type)]
struct UserProfile {
    #[serde(skip)]
    id: UserId,
    username: Option<String>, // This is non-null. Workaround for deriving sqlx::Type.
    bio: Option<String>,
    image: Option<String>,
    following: bool,
}

#[derive(Debug, Deserialize)]
pub struct Login {
    user: LoginUser,
}

#[derive(Debug, Deserialize, Validate)]
struct LoginUser {
    #[validate(
        length(min = 1, message = "email can't be blank"),
        email(message = "invalid email address")
    )]
    email: String,
    #[validate(length(min = 1, message = "password can't be blank"))]
    password: String,
}

pub async fn login(
    State(pool): State<PgPool>,
    State(key): State<EncodingKey>,
    Json(Login { user }): Json<Login>,
) -> AppResult<impl IntoResponse> {
    user.validate()?;

    let mut conn = pool.acquire().await.unwrap();

    let user_auth = sqlx::query_as!(
        UserAuth,
        "SELECT *, NULL AS token FROM users WHERE email = $1",
        user.email
    )
    .fetch_optional(&mut conn)
    .await?;

    let Some(mut user_auth) = user_auth else {
        Err(AppError::ForbiddenError(json!({
            "email or password": "is invalid"
        })))?
    };

    let hash =
        password_hash::PasswordHash::new(&user_auth.hash).map_err(|err| anyhow::anyhow!(err))?;

    hash.verify_password(&[&argon2::Argon2::default()], &user.password)
        .map_err(|err| {
            log::error!("err: {:?}", err);
            AppError::ForbiddenError(json!({
                "email or password": "is invalid"
            }))
        })?;

    user_auth.token = Some(auth::generate_jwt(user_auth.id, &key)?);

    Ok(Json(json!({ "user": user_auth })))
}

fn hash_password(password: impl AsRef<[u8]>) -> AppResult<String> {
    let salt = password_hash::SaltString::generate(&mut rand::thread_rng());

    let hash = password_hash::PasswordHash::generate(
        argon2::Argon2::default(),
        password.as_ref(),
        salt.as_str(),
    )
    .map_err(|err| anyhow::anyhow!(err))?
    .to_string();
    Ok(hash)
}

#[derive(Deserialize)]
pub struct Registration {
    user: RegistrationUser,
}

#[derive(Deserialize, Validate)]
struct RegistrationUser {
    #[validate(
        non_control_character(message = "user name can't contain non-ascii charactors"),
        length(min = 1, message = "user name can't be blank"),
        length(max = 64, message = "too long user name")
    )]
    username: String,

    #[validate(
        length(min = 1, message = "email can't be blank"),
        length(max = 64, message = "too long email address"),
        email(message = "invalid email address")
    )]
    email: String,

    #[validate(
        non_control_character(message = "password can't contain non-ascii charactors"),
        length(min = 8, message = "password must be at least 8 characters long"),
        length(max = 64, message = "too long password")
    )]
    password: String,
}

pub async fn registration(
    State(pool): State<PgPool>,
    State(key): State<EncodingKey>,
    Json(Registration { user }): Json<Registration>,
) -> AppResult<impl IntoResponse> {
    user.validate()?;

    let hash = hash_password(user.password)?;

    let mut conn = pool.acquire().await.unwrap();

    let mut user_auth = sqlx::query_as!(
        UserAuth,
        r#"
        INSERT INTO users (username, email, hash)
        VALUES ($1, $2, $3)
        RETURNING *, NULL AS token
        "#,
        user.username,
        user.email,
        hash
    )
    .fetch_one(&mut conn)
    .await?;

    user_auth.token = Some(auth::generate_jwt(user_auth.id, &key)?);

    Ok(Json(json!({ "user": user_auth })))
}

fn verify_token(token: &str, key: &DecodingKey) -> AppResult<UserId> {
    let claim = auth::verify_jwt(token, &key)?;
    Ok(claim.user_id)
}

async fn get_user(user_id: UserId, pool: &PgPool) -> AppResult<UserAuth> {
    let mut conn = pool.acquire().await.unwrap();

    let user_auth = sqlx::query_as!(
        UserAuth,
        "SELECT *, NULL AS token FROM users WHERE id = $1",
        user_id
    )
    .fetch_one(&mut conn)
    .await?;

    Ok(user_auth)
}

async fn get_user_profile(
    pool: &PgPool,
    username: &str,
    req_user_id: Option<UserId>,
) -> AppResult<UserProfile> {
    let user = sqlx::query_as!(
        UserProfile,
        r#"
        SELECT
            users.id, users.username AS "username?", users.bio, users.image,
            ($2::INT4 IS NOT NULL AND EXISTS (
                SELECT 1 FROM follows
                WHERE follows.follower_id = $2 AND follows.followee_id = users.id
            )) AS "following!"
        FROM users WHERE username = $1
        "#,
        username,
        req_user_id
    )
    .fetch_one(&mut pool.acquire().await.unwrap())
    .await?;

    Ok(user)
}

async fn auth_user(pool: &PgPool, token: &str, key: &DecodingKey) -> AppResult<UserAuth> {
    let user_id = verify_token(token, key)?;
    let mut user = get_user(user_id, pool).await?;
    user.token = Some(token.to_string());
    Ok(user)
}

pub async fn get_current_user(
    State(pool): State<PgPool>,
    State(key): State<DecodingKey>,
    TypedHeader(Authorization(token)): TypedHeader<Authorization<JWTToken>>,
) -> AppResult<impl IntoResponse> {
    let user = auth_user(&pool, &token.0, &key).await?;
    Ok(Json(json!({ "user": user })))
}

#[derive(Debug, Deserialize)]
pub struct UpdateUser {
    user: UpdateUserData,
}

#[derive(Debug, Deserialize, Validate)]
struct UpdateUserData {
    #[validate(email)]
    email: Option<String>,
    #[validate(non_control_character, length(min = 1, max = 64))]
    username: Option<String>,
    #[validate(non_control_character, length(min = 8, max = 64))]
    password: Option<String>,
    bio: Option<String>,
    image: Option<String>,
}

pub async fn update_user(
    State(pool): State<PgPool>,
    State(key): State<DecodingKey>,
    TypedHeader(Authorization(token)): TypedHeader<Authorization<JWTToken>>,
    Json(UpdateUser { user: data }): Json<UpdateUser>,
) -> AppResult<impl IntoResponse> {
    let user = auth_user(&pool, &token.0, &key).await?;

    let hash = data
        .password
        .map(|password| hash_password(password))
        .transpose()?;

    let mut updated_user = sqlx::query_as!(
        UserAuth,
        "UPDATE users
            SET (email, username, hash, bio, image) = 
                (
                    COALESCE($1, email),
                    COALESCE($2, username),
                    COALESCE($3, hash),
                    COALESCE($4, bio),
                    COALESCE($5, image)
                )
            WHERE id = $6
        RETURNING *, NULL AS token
        ",
        data.email,
        data.username,
        hash,
        data.bio,
        data.image,
        user.id
    )
    .fetch_one(&mut pool.acquire().await.unwrap())
    .await?;

    updated_user.token = user.token;

    Ok(Json(json!({ "user": updated_user })))
}

pub async fn get_profile(
    State(pool): State<PgPool>,
    State(key): State<DecodingKey>,
    Path(username): Path<String>,
    token: Option<TypedHeader<Authorization<JWTToken>>>,
) -> AppResult<impl IntoResponse> {
    let user_id = token
        .map(|TypedHeader(Authorization(token))| verify_token(&token.0, &key))
        .transpose()?;

    let profile = get_user_profile(&pool, &username, user_id).await?;

    Ok(Json(json!({ "profile": profile })))
}

pub async fn follow_user(
    State(pool): State<PgPool>,
    State(key): State<DecodingKey>,
    Path(username): Path<String>,
    TypedHeader(Authorization(token)): TypedHeader<Authorization<JWTToken>>,
) -> AppResult<impl IntoResponse> {
    let follower_id = verify_token(&token.0, &key)?;
    let mut followee = get_user_profile(&pool, &username, Some(follower_id)).await?;

    sqlx::query!(
        "
        INSERT INTO follows (follower_id, followee_id)
        VALUES ($1, $2)
        ",
        follower_id,
        followee.id
    )
    .execute(&mut pool.acquire().await.unwrap())
    .await?;

    followee.following = true;

    Ok(Json(json!({ "profile": followee })))
}

pub async fn unfollow_user(
    State(pool): State<PgPool>,
    State(key): State<DecodingKey>,
    Path(username): Path<String>,
    TypedHeader(Authorization(token)): TypedHeader<Authorization<JWTToken>>,
) -> AppResult<impl IntoResponse> {
    let follower_id = verify_token(&token.0, &key)?;
    let mut followee = get_user_profile(&pool, &username, Some(follower_id)).await?;
    followee.following = false;

    sqlx::query!(
        "
        DELETE FROM follows
        WHERE (follower_id, followee_id) = ($1, $2)
        ",
        follower_id,
        followee.id
    )
    .execute(&mut pool.acquire().await.unwrap())
    .await?;

    followee.following = false;

    Ok(Json(json!({ "profile": followee })))
}

struct ArticleWithCount {
    id: i32,
    slug: String,
    title: String,
    description: String,
    body: String,
    tag_list: Vec<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    favorited: bool,
    favorites_count: i64,
    author: UserProfile,
    count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Article {
    #[serde(skip)]
    id: i32,
    slug: String,
    title: String,
    description: String,
    body: String,
    tag_list: Vec<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    favorited: bool,
    favorites_count: i64,
    author: UserProfile,
}

#[derive(Debug, Deserialize)]
pub struct ListArticlesQuery {
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    favorited: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
}

pub async fn list_articles(
    State(pool): State<PgPool>,
    State(key): State<DecodingKey>,
    Query(query): Query<ListArticlesQuery>,
    token: Option<TypedHeader<Authorization<JWTToken>>>,
) -> AppResult<impl IntoResponse> {
    let user_id = token
        .map(|token| verify_token(&token.0 .0 .0, &key))
        .transpose()?;

    let articles = sqlx::query_as!(
        ArticleWithCount,
        r#"
        SELECT
            articles.id,
            articles.slug,
            articles.title,
            articles.description,
            articles.body,
            articles.created_at,
            articles.updated_at,
            COALESCE(
                (SELECT
                    array_agg(tags.name ORDER BY tags.name ASC)
                    FROM article_tags
                    INNER JOIN tags ON article_tags.tag_id = tags.id
                    WHERE article_tags.article_id = articles.id
                ),
                '{}'::VARCHAR[]
            ) AS "tag_list!",
            ($6::INT4 IS NOT NULL AND EXISTS (
                SELECT 1 FROM article_favs
                WHERE article_favs.article_id = articles.id
                AND article_favs.user_id = $6
            )) AS "favorited!",
            (SELECT COUNT(*)
                FROM article_favs
                WHERE article_favs.article_id = articles.id
            ) AS "favorites_count!",
            (
                users.id,
                users.username,
                users.bio,
                users.image,
                ($6 IS NOT NULL AND EXISTS (
                    SELECT 1 FROM follows
                    WHERE follows.follower_id = $6
                    AND follows.followee_id = users.id
                ))
            ) AS "author!: UserProfile",
            COUNT(*) OVER() AS "count!"
        FROM articles
        INNER JOIN users ON articles.author_id = users.id
        WHERE
            ($1::VARCHAR IS NULL OR users.username = $1)
            AND ($2::VARCHAR IS NULL OR EXISTS (
                SELECT 1 FROM article_favs
                INNER JOIN users ON article_favs.user_id = users.id
                WHERE article_favs.article_id = articles.id AND users.username = $2
            ))
            AND ($3::VARCHAR IS NULL OR EXISTS (
                SELECT 1 FROM article_tags
                INNER JOIN tags ON article_tags.tag_id = tags.id
                WHERE article_tags.article_id = articles.id AND tags.name = $3
            ))
        ORDER BY created_at DESC
        LIMIT $4 OFFSET $5
        "#,
        query.author,
        query.favorited,
        query.tag,
        query.limit.unwrap_or(20) as i64,
        query.offset.unwrap_or(0) as i64,
        user_id,
    )
    .fetch_all(&mut pool.acquire().await.unwrap())
    .await?;

    Ok(Json(json!({
        "articlesCount": articles.iter().next().map(|a| a.count).unwrap_or(0),
        "articles": articles.into_iter().map(|article| Article {
            id: article.id,
            slug: article.slug,
            title: article.title,
            description: article.description,
            body: article.body,
            tag_list: article.tag_list,
            created_at: article.created_at,
            updated_at: article.updated_at,
            favorited: article.favorited,
            favorites_count: article.favorites_count,
            author: article.author,
        }).collect::<Vec<_>>(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct FeedArticlesQuery {
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
}

pub async fn feed_articles(
    State(pool): State<PgPool>,
    State(key): State<DecodingKey>,
    Query(query): Query<FeedArticlesQuery>,
    TypedHeader(Authorization(token)): TypedHeader<Authorization<JWTToken>>,
) -> AppResult<impl IntoResponse> {
    let user_id = verify_token(&token.0, &key)?;

    let articles = sqlx::query_as!(
        ArticleWithCount,
        r#"
        SELECT
            articles.id,
            articles.slug,
            articles.title,
            articles.description,
            articles.body,
            articles.created_at,
            articles.updated_at,
            COALESCE(
                (SELECT
                    array_agg(tags.name ORDER BY tags.name ASC)
                    FROM article_tags
                    INNER JOIN tags ON article_tags.tag_id = tags.id
                    WHERE article_tags.article_id = articles.id
                ),
                '{}'::VARCHAR[]
            ) AS "tag_list!",
            ($1::INT4 IS NOT NULL AND EXISTS (
                SELECT 1 FROM article_favs
                WHERE article_favs.article_id = articles.id
                AND article_favs.user_id = $1
            )) AS "favorited!",
            (SELECT COUNT(*)
                FROM article_favs
                WHERE article_favs.article_id = articles.id
            ) AS "favorites_count!",
            (
                users.id,
                users.username,
                users.bio,
                users.image,
                TRUE
            ) AS "author!: UserProfile",
            COUNT(*) OVER() AS "count!"
        FROM articles
        INNER JOIN users ON articles.author_id = users.id
        WHERE
            EXISTS (
                SELECT 1 FROM follows
                INNER JOIN users ON follows.followee_id = users.id
                WHERE follows.follower_id = $1
                    AND follows.followee_id = articles.author_id 
            )
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        "#,
        user_id,
        query.limit.unwrap_or(20) as i64,
        query.offset.unwrap_or(0) as i64,
    )
    .fetch_all(&mut pool.acquire().await.unwrap())
    .await?;

    Ok(Json(json!({
        "articlesCount": articles.iter().next().map(|a| a.count).unwrap_or(0),
        "articles": articles.into_iter().map(|article| Article {
            id: article.id,
            slug: article.slug,
            title: article.title,
            description: article.description,
            body: article.body,
            tag_list: article.tag_list,
            created_at: article.created_at,
            updated_at: article.updated_at,
            favorited: article.favorited,
            favorites_count: article.favorites_count,
            author: article.author,
        }).collect::<Vec<_>>(),
    })))
}

async fn get_article_by_slug(
    pool: &PgPool,
    slug: &str,
    user_id: Option<UserId>,
) -> AppResult<Article> {
    let article: Article = sqlx::query_as!(
        Article,
        r#"
        SELECT
            articles.id,
            articles.slug,
            articles.title,
            articles.description,
            articles.body,
            articles.created_at,
            articles.updated_at,
            COALESCE(
                (SELECT
                    array_agg(tags.name ORDER BY tags.name ASC)
                    FROM article_tags
                    INNER JOIN tags ON article_tags.tag_id = tags.id
                    WHERE article_tags.article_id = articles.id
                ),
                '{}'::VARCHAR[]
            ) AS "tag_list!",
            ($2::INT4 IS NOT NULL AND EXISTS (
                SELECT 1 FROM article_favs
                WHERE article_favs.article_id = articles.id
                AND article_favs.user_id = $2
            )) AS "favorited!",
            (SELECT COUNT(*)
                FROM article_favs
                WHERE article_favs.article_id = articles.id
            ) AS "favorites_count!",
            (
                users.id,
                users.username,
                users.bio,
                users.image,
                ($2 IS NOT NULL AND EXISTS (
                    SELECT 1 FROM follows
                    WHERE follows.follower_id = $2
                    AND follows.followee_id = users.id
                ))
            ) AS "author!: UserProfile"
        FROM articles
        INNER JOIN users ON articles.author_id = users.id
        WHERE articles.slug = $1
        "#,
        slug,
        user_id,
    )
    .fetch_one(&mut pool.acquire().await.unwrap())
    .await?;

    Ok(article)
}

pub async fn get_article(
    State(pool): State<PgPool>,
    State(key): State<DecodingKey>,
    Path(slug): Path<String>,
    token: Option<TypedHeader<Authorization<JWTToken>>>,
) -> AppResult<impl IntoResponse> {
    let user_id = token
        .map(|token| verify_token(&token.0 .0 .0, &key))
        .transpose()?;
    Ok(Json(
        json!({ "article": get_article_by_slug(&pool, &slug, user_id).await? }),
    ))
}

#[derive(Deserialize)]
pub struct CreateArticle {
    article: CreateArticleData,
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
struct CreateArticleData {
    #[validate(length(min = 1, message = "title can't be blank"))]
    title: String,
    #[validate(length(min = 1, message = "description can't be blank"))]
    description: String,
    #[validate(length(min = 1, message = "body can't be blank"))]
    body: String,
    #[serde(default)]
    tag_list: Vec<String>,
}

pub async fn create_article(
    State(pool): State<PgPool>,
    State(key): State<DecodingKey>,
    TypedHeader(Authorization(token)): TypedHeader<Authorization<JWTToken>>,
    Json(CreateArticle { article }): Json<CreateArticle>,
) -> AppResult<impl IntoResponse> {
    article.validate()?;

    let user_id = verify_token(&token.0, &key)?;

    let slug = slug::slugify(&article.title);
    let tags = article.tag_list;

    let mut article: Article = sqlx::query_as!(
        Article,
        r#"
            WITH article AS (
                INSERT INTO articles (slug, title, description, body, author_id)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING *
            )
            SELECT
                article.id,
                article.slug,
                article.title,
                article.description,
                article.body,
                article.created_at,
                article.updated_at,
                FALSE AS "favorited!",
                '{}'::VARCHAR[] AS "tag_list!",
                CAST(0 as INT8) AS "favorites_count!",
                (
                    users.id,
                    users.username,
                    users.bio,
                    users.image,
                    EXISTS (
                        SELECT 1 FROM follows
                        WHERE follows.follower_id = $5
                        AND follows.followee_id = users.id
                    )
                ) AS "author!: UserProfile"
            FROM article
            INNER JOIN users ON users.id = article.author_id
        "#,
        slug,
        article.title,
        article.description,
        article.body,
        user_id
    )
    .fetch_one(&mut pool.acquire().await.unwrap())
    .await?;

    sqlx::query!(
        "
        INSERT INTO tags (name)
        SELECT * FROM UNNEST($1::TEXT[])
        ON CONFLICT DO NOTHING
        ",
        &tags[..]
    )
    .execute(&mut pool.acquire().await.unwrap())
    .await?;

    sqlx::query!(
        "
        INSERT INTO article_tags (article_id, tag_id)
        SELECT $1, tags.id FROM tags WHERE tags.name = ANY($2)
        ",
        article.id,
        &tags[..],
    )
    .execute(&mut pool.acquire().await.unwrap())
    .await?;

    article.tag_list = tags;

    Ok(Json(json!({ "article": article })))
}

#[derive(Deserialize)]
pub struct UpdateArticle {
    article: UpdateArticleData,
}

#[derive(Deserialize)]
struct UpdateArticleData {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    body: Option<String>,
}

pub async fn update_article(
    State(pool): State<PgPool>,
    State(key): State<DecodingKey>,
    Path(slug): Path<String>,
    TypedHeader(Authorization(token)): TypedHeader<Authorization<JWTToken>>,
    Json(UpdateArticle { article }): Json<UpdateArticle>,
) -> AppResult<impl IntoResponse> {
    let user_id = verify_token(&token.0, &key)?;

    let article: Article = sqlx::query_as!(
        Article,
        r#"
        WITH article AS (
            UPDATE articles
            SET
                title = COALESCE($1, title),
                description = COALESCE($2, description),
                body = COALESCE($3, body)
            WHERE slug = $4 AND author_id = $5
            RETURNING *
        )
        SELECT
            article.id,
            article.slug,
            article.title,
            article.description,
            article.body,
            article.created_at,
            article.updated_at,
            COALESCE(
                (SELECT
                    array_agg(tags.name ORDER BY tags.name ASC)
                    FROM article_tags
                    INNER JOIN tags ON article_tags.tag_id = tags.id
                    WHERE article_tags.article_id = article.id
                ),
                '{}'::VARCHAR[]
            ) AS "tag_list!",
            ($5 IS NOT NULL AND EXISTS (
                SELECT  FROM article_favs
                WHERE article_favs.article_id = article.id
                AND article_favs.user_id = $5
            )) AS "favorited!",
            (SELECT COUNT(*)
                FROM article_favs
                WHERE article_favs.article_id = article.id
            ) AS "favorites_count!",    
            (
                users.id,
                users.username,
                users.bio,
                users.image,
                EXISTS (
                    SELECT 1 FROM follows
                    WHERE follows.follower_id = $5
                    AND follows.followee_id = users.id
                )
            ) AS "author!: UserProfile"
        FROM article
        INNER JOIN users ON users.id = article.author_id
        "#,
        article.title,
        article.description,
        article.body,
        slug,
        user_id,
    )
    .fetch_one(&mut pool.acquire().await.unwrap())
    .await?;

    Ok(Json(json!({ "article": article })))
}

pub async fn delete_article(
    State(pool): State<PgPool>,
    State(key): State<DecodingKey>,
    Path(slug): Path<String>,
    TypedHeader(Authorization(token)): TypedHeader<Authorization<JWTToken>>,
) -> AppResult<impl IntoResponse> {
    let user_id = verify_token(&token.0, &key)?;

    sqlx::query!(
        "
        DELETE FROM articles
        WHERE slug = $1 AND author_id = $2
        ",
        slug,
        user_id
    )
    .execute(&mut pool.acquire().await.unwrap())
    .await?;

    Ok(Json(json!({})))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Comment {
    id: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    body: String,
    author: UserProfile,
}

#[derive(Deserialize)]
pub struct AddComment {
    comment: AddCommentData,
}

#[derive(Deserialize)]
struct AddCommentData {
    body: String,
}

pub async fn add_comment(
    State(pool): State<PgPool>,
    State(key): State<DecodingKey>,
    Path(slug): Path<String>,
    TypedHeader(Authorization(token)): TypedHeader<Authorization<JWTToken>>,
    Json(AddComment { comment }): Json<AddComment>,
) -> AppResult<impl IntoResponse> {
    let user_id = verify_token(&token.0, &key)?;

    let comment: Comment = sqlx::query_as!(
        Comment,
        r#"
        WITH comment AS (
            INSERT INTO comments (body, article_id, author_id)
            VALUES ($1, (SELECT id FROM articles WHERE slug = $2), $3)
            RETURNING *
        )
        SELECT
            comment.id,
            comment.created_at,
            comment.updated_at,
            comment.body,
            (
                users.id,
                users.username,
                users.bio,
                users.image,
                ($3 IS NOT NULL AND EXISTS (
                    SELECT 1 FROM follows
                    WHERE follows.follower_id = $3
                    AND follows.followee_id = users.id
                ))
            ) AS "author!: UserProfile"
        FROM comment INNER JOIN users ON users.id = comment.author_id
        "#,
        comment.body,
        slug,
        user_id,
    )
    .fetch_one(&mut pool.acquire().await.unwrap())
    .await?;

    Ok(Json(json!({ "comment": comment })))
}

pub async fn get_comments(
    State(pool): State<PgPool>,
    State(key): State<DecodingKey>,
    Path(slug): Path<String>,
    token: Option<TypedHeader<Authorization<JWTToken>>>,
) -> AppResult<impl IntoResponse> {
    let user_id = token
        .map(|token| verify_token(&token.0 .0 .0, &key))
        .transpose()?;
    let comments: Vec<Comment> = sqlx::query_as!(
        Comment,
        r#"
        SELECT
            comments.id,
            comments.created_at,
            comments.updated_at,
            comments.body,
            (
                users.id,
                users.username,
                users.bio,
                users.image,
                ($2::INT4 IS NOT NULL AND EXISTS (
                    SELECT 1 FROM follows
                    WHERE follows.follower_id = $2
                    AND follows.followee_id = users.id
                ))
            ) AS "author!: UserProfile"
        FROM comments
        INNER JOIN users ON users.id = comments.author_id
        WHERE comments.article_id = (SELECT id FROM articles WHERE slug = $1)
        ORDER BY comments.created_at DESC
        "#,
        slug,
        user_id,
    )
    .fetch_all(&mut pool.acquire().await.unwrap())
    .await?;

    Ok(Json(json!({ "comments": comments })))
}

#[derive(Deserialize)]
pub struct DeleteCommentPath {
    slug: String,
    id: i32,
}

pub async fn delete_comment(
    State(pool): State<PgPool>,
    State(key): State<DecodingKey>,
    Path(DeleteCommentPath { slug, id }): Path<DeleteCommentPath>,
    TypedHeader(Authorization(token)): TypedHeader<Authorization<JWTToken>>,
) -> AppResult<impl IntoResponse> {
    let user_id = verify_token(&token.0, &key)?;

    sqlx::query!(
        "
        DELETE FROM comments
        WHERE comments.id = $1
            AND comments.article_id = (SELECT id FROM articles WHERE slug = $2)
            AND comments.author_id = $3
        ",
        id,
        slug,
        user_id,
    )
    .execute(&mut pool.acquire().await.unwrap())
    .await?;

    Ok(Json(json!({})))
}

pub async fn favorite_article(
    State(pool): State<PgPool>,
    State(key): State<DecodingKey>,
    Path(slug): Path<String>,
    TypedHeader(Authorization(token)): TypedHeader<Authorization<JWTToken>>,
) -> AppResult<impl IntoResponse> {
    let user_id = verify_token(&token.0, &key)?;

    sqlx::query!(
        "
        INSERT INTO article_favs (article_id, user_id)
        SELECT articles.id, $2
            FROM articles
            WHERE articles.slug = $1
        ",
        slug,
        user_id
    )
    .execute(&mut pool.acquire().await.unwrap())
    .await?;

    let article = get_article_by_slug(&pool, &slug, Some(user_id)).await?;

    Ok(Json(json!({ "article": article })))
}

pub async fn unfavorite_article(
    State(pool): State<PgPool>,
    State(key): State<DecodingKey>,
    Path(slug): Path<String>,
    TypedHeader(Authorization(token)): TypedHeader<Authorization<JWTToken>>,
) -> AppResult<impl IntoResponse> {
    let user_id = verify_token(&token.0, &key)?;

    sqlx::query!(
        "
        DELETE FROM article_favs
            WHERE article_favs.article_id = ANY(
                SELECT articles.id FROM articles
                WHERE articles.slug = $1
            )
            AND article_favs.user_id = $2
        ",
        slug,
        user_id,
    )
    .execute(&mut pool.acquire().await.unwrap())
    .await?;

    let article = get_article_by_slug(&pool, &slug, Some(user_id)).await?;

    Ok(Json(json!({ "article": article })))
}

struct Tag {
    name: String,
}

pub async fn get_tags(State(pool): State<PgPool>) -> AppResult<impl IntoResponse> {
    let tags: Vec<Tag> = sqlx::query_as!(
        Tag,
        r"
        SELECT tags.name
        FROM tags
        INNER JOIN article_tags ON article_tags.tag_id = tags.id
        GROUP BY tags.name
        ORDER BY COUNT(article_tags.tag_id) DESC
        LIMIT 10
        "
    )
    .fetch_all(&mut pool.acquire().await.unwrap())
    .await?;

    let tags = tags
        .into_iter()
        .map(|tag| tag.name)
        .collect::<Vec<String>>();

    Ok(Json(json!({ "tags": tags })))
}
