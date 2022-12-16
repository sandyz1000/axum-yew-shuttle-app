use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use fake::{
    faker::{
        internet::en::{FreeEmail, Password},
        lorem::en::{Paragraphs, Sentence, Words},
        name::en::Name,
    },
    Dummy, Fake,
};
use indicatif::ProgressIterator;
use rand::Rng;
use reqwest::{
    blocking::{Client, RequestBuilder},
    header::AUTHORIZATION,
};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};

const USER_NUM: usize = 20;
const ARTICLE_NUM: usize = 300;
const COMMENT_NUM: usize = 1000;
const FAVORITE_NUM: usize = 500;

#[derive(Debug, Dummy)]
struct User {
    #[dummy(faker = "Name()")]
    name: String,
    #[dummy(faker = "FreeEmail()")]
    email: String,
    #[dummy(faker = "Password(8..16)")]
    password: String,
    #[dummy(faker = "Sentence(5..11)")]
    bio: String,
}

#[derive(Debug, Dummy)]
struct Article {
    #[dummy(faker = "DummyTitle")]
    title: String,
    #[dummy(faker = "Sentence(5..11)")]
    description: String,
    #[dummy(faker = "DummyBody")]
    body: String,
    #[dummy(faker = "Words(0..6)")]
    tag_list: Vec<String>,
}

struct DummyTitle;

impl Dummy<DummyTitle> for String {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_config: &DummyTitle, rng: &mut R) -> Self {
        let words: Vec<String> = Words(2..11).fake_with_rng(rng);
        words.join(" ")
    }
}

struct DummyBody;

impl Dummy<DummyBody> for String {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_config: &DummyBody, rng: &mut R) -> Self {
        let paragraphs: Vec<String> = Paragraphs(1..6).fake_with_rng(rng);
        paragraphs.join("\n\n")
    }
}

#[derive(Debug, Dummy)]
struct Comment {
    #[dummy(faker = "Sentence(10..21)")]
    body: String,
}

#[derive(Debug)]
struct Follow {
    follower_id: usize,
    followee_id: usize,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct UserAuth {
    username: String,
    email: String,
    token: String,
    bio: Option<String>,
    image: Option<String>,
}

#[derive(Deserialize)]
struct UserAuthResp {
    user: UserAuth,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct Profile {
    username: String,
    bio: Option<String>,
    image: Option<String>,
    following: bool,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct ProfileResp {
    profile: Profile,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SingleArticle {
    slug: String,
    title: String,
    description: String,
    body: String,
    tag_list: Vec<String>,
    created_at: String,
    updated_at: String,
    favorited: bool,
    favorites_count: usize,
    author: Profile,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct SingleArticleResp {
    article: SingleArticle,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SingleComment {
    id: usize,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    body: String,
    author: Profile,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct SingleCommentResp {
    comment: SingleComment,
}

fn main() -> anyhow::Result<()> {
    let users = fake::vec![User; USER_NUM];

    let mut follows = vec![];

    for follower in 0..users.len() {
        for followee in 0..users.len() {
            if rand::thread_rng().gen_bool(0.2) {
                follows.push(Follow {
                    follower_id: follower,
                    followee_id: followee,
                });
            }
        }
    }

    let articles = fake::vec![Article; ARTICLE_NUM];

    let comments = fake::vec![Comment; COMMENT_NUM];

    let apiurl = std::env::var("APIURL").unwrap_or("http://localhost:8000/api".to_string());

    let client = Client::new();

    println!("Initializing database");
    let _resp: Value = get_response(client.post(format!("{apiurl}/initialize")))?;

    let style = indicatif::ProgressStyle::default_bar().template(
        "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos:>4}/{len:4} {eta:6} {msg}",
    )?;

    println!("Registering users");

    let mut user_auth = vec![];

    for user in users.iter().progress_with_style(style.clone()) {
        let resp: UserAuthResp =
            get_response(client.post(format!("{apiurl}/users")).json(&json!({
                "user": {
                    "username": user.name,
                    "email": user.email,
                    "password": user.password,
                }
            })))?;

        user_auth.push(resp.user);
    }

    println!("Setting user profiles");
    for user_id in 0..USER_NUM {
        let user = &users[user_id];
        let user_auth = &user_auth[user_id];

        let _resp: UserAuthResp = get_response(
            client
                .put(format!("{apiurl}/user"))
                .auth(&user_auth.token)
                .json(&json!({
                    "user": {
                        "bio": user.bio,
                    }
                })),
        )?;
    }

    println!("Following");

    for follow in follows.iter().progress_with_style(style.clone()) {
        let followee_name = &user_auth[follow.followee_id].username;
        let follower_token = &user_auth[follow.follower_id].token;

        let _resp: ProfileResp = get_response(
            client
                .post(format!("{apiurl}/profiles/{followee_name}/follow"))
                .auth(&follower_token),
        )?;
    }

    println!("Adding articles");

    for article in articles.iter().progress_with_style(style.clone()) {
        let author_id = rand::thread_rng().gen_range(0..user_auth.len());
        let author_token = &user_auth[author_id].token;

        let _resp: SingleArticleResp = get_response(
            client
                .post(format!("{apiurl}/articles"))
                .auth(&author_token)
                .json(&json!({
                    "article": {
                        "title": article.title,
                        "description": article.description,
                        "body": article.body,
                        "tagList": article.tag_list,
                    }
                })),
        )?;
    }

    println!("Adding comments");

    for comment in comments.iter().progress_with_style(style.clone()) {
        let author_id = rand::thread_rng().gen_range(0..user_auth.len());
        let author_token = &user_auth[author_id].token;

        let article_id = rand::thread_rng().gen_range(0..articles.len());
        let article_slug = slug::slugify(&articles[article_id].title);

        let _resp: SingleCommentResp = get_response(
            client
                .post(format!("{apiurl}/articles/{article_slug}/comments"))
                .auth(&author_token)
                .json(&json!({
                    "comment": {
                        "body": comment.body,
                    }
                })),
        )?;
    }

    println!("Add favorites");

    let mut favorited = BTreeSet::new();

    for _ in (0..FAVORITE_NUM).progress_with_style(style.clone()) {
        let user_id = rand::thread_rng().gen_range(0..user_auth.len());
        let user_token = &user_auth[user_id].token;

        let article_id = rand::thread_rng().gen_range(0..articles.len());
        let article_slug = slug::slugify(&articles[article_id].title);

        if favorited.contains(&(user_id, article_id)) {
            continue;
        }
        favorited.insert((user_id, article_id));

        let _resp: SingleArticleResp = get_response(
            client
                .post(format!("{apiurl}/articles/{article_slug}/favorite"))
                .auth(&user_token),
        )?;
    }

    Ok(())
}

trait RequestBuilderExt {
    fn auth(self, token: &str) -> Self;
}

impl RequestBuilderExt for RequestBuilder {
    fn auth(self, token: &str) -> Self {
        self.header(AUTHORIZATION, format!("Token {token}"))
    }
}

fn get_response<T: DeserializeOwned>(req: RequestBuilder) -> anyhow::Result<T> {
    for _ in 0..5 {
        let resp = req.try_clone().unwrap().send()?;

        if resp.status().is_success() {
            return Ok(resp.json()?);
        }

        if resp.status().as_u16() == 503 {
            println!("Service Unavailable, retrying...");
            continue;
        }

        return Err(anyhow::anyhow!("request failed: {}", resp.text()?));
    }

    Err(anyhow::anyhow!("request failed after 5 retries"))
}
