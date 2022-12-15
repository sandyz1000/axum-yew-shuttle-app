use std::rc::Rc;

use chrono::{DateTime, Local};
use serde_json::json;
use web_sys::{Element, HtmlTextAreaElement};
use yew::prelude::*;
use yew_hooks::prelude::*;
use yew_router::prelude::*;

use crate::{
    api::{ApiError, ApiRequest, ArticleResp, Comment, CommentResp, CommentsResp, UserProfileResp},
    route::Route,
};

#[derive(PartialEq, Properties)]
pub struct ArticleProps {
    pub slug: String,
}

#[function_component]
pub fn Article(props: &ArticleProps) -> Html {
    let ArticleProps { slug } = props;

    let auth = use_context::<crate::auth::AuthContext>().unwrap();

    let article = use_state_ptr_eq(|| None);

    let reload_article = use_async_with_options(
        {
            let slug = slug.clone();
            let auth = auth.clone();
            let article = article.clone();
            async move {
                let a: ArticleResp = ApiRequest::get(format!("/api/articles/{}", slug))
                    .auth(auth.user())
                    .json_response()
                    .await?;

                article.set(Some(a.article));

                Ok::<_, Rc<ApiError>>(())
            }
        },
        UseAsyncOptions::enable_auto(),
    );

    let comments = use_state_ptr_eq(|| vec![]);

    let reload_comments = use_async_with_options(
        {
            let slug = slug.clone();
            let auth = auth.clone();
            let comments = comments.clone();
            async move {
                let c: CommentsResp = ApiRequest::get(format!("/api/articles/{slug}/comments"))
                    .auth(auth.user())
                    .json_response()
                    .await?;

                comments.set(c.comments);

                Ok::<_, Rc<ApiError>>(())
            }
        },
        UseAsyncOptions::enable_auto(),
    );

    let comment_ref = use_node_ref();

    let post_comment = use_async({
        let comment_ref = comment_ref.clone();
        let reload_comments = reload_comments.clone();
        let slug = slug.clone();
        let auth = auth.clone();

        async move {
            let comment = comment_ref.cast::<HtmlTextAreaElement>().unwrap().value();
            let comment = comment.trim_start();

            if comment == "" {
                return Ok(());
            }

            let _resp: CommentResp = ApiRequest::post(format!("/api/articles/{slug}/comments"))
                .auth(auth.user())
                .json(&json!({
                    "comment": {
                        "body": comment
                    }
                }))
                .json_response()
                .await?;

            reload_comments.run();

            Ok::<_, Rc<ApiError>>(())
        }
    });

    let delete_comment_id = use_state_ptr_eq(|| None);

    let delete_comment = use_async({
        let reload_comments = reload_comments.clone();
        let comment_id = delete_comment_id.clone();
        let slug = slug.clone();
        let auth = auth.clone();

        async move {
            let Some(comment_id) = *comment_id else {
                    return Ok(());
                };
            let _resp: serde_json::Value =
                ApiRequest::delete(format!("/api/articles/{slug}/comments/{comment_id}"))
                    .auth(auth.user())
                    .json_response()
                    .await?;

            reload_comments.run();

            Ok::<_, Rc<ApiError>>(())
        }
    });

    let on_delete_comment = {
        let delete_comment_id = delete_comment_id.clone();
        let delete_comment = delete_comment.clone();
        Callback::from(move |id| {
            delete_comment_id.set(Some(id));
            delete_comment.run();
        })
    };

    use_effect_with_deps(move |_| reload_article.run(), auth.clone());
    use_effect_with_deps(move |_| reload_comments.run(), auth.clone());

    html! {
        <div class="article-page">
        <div class="banner">
          <div class="container">
            <ArticleBanner article={article.clone()} />
          </div>
        </div>

        <div class="container page">
          <div class="row article-content">
            <div class="col-md-12">
                <ArticleContent article={article.clone()} />
            </div>
          </div>

          <hr />

          <div class="article-actions">
            <ArticleMeta article={article.clone()} />
          </div>

          <div class="row">
            <div class="col-xs-12 col-md-8 offset-md-2">
              if auth.is_authorized() {
                <form class="card comment-form">
                    <div class="card-block">
                    <textarea ref={comment_ref} class="form-control" placeholder="Write a comment..." rows="3"></textarea>
                    </div>
                    <div class="card-footer">
                    <img src={auth.user().map(|u| u.image().to_string())} class="comment-author-img" />
                    <button onclick={move |_| post_comment.run()} class="btn btn-sm btn-primary">{"Post Comment"}</button>
                    </div>
                </form>
              } else {
                <p>
                    <Link<Route> to={Route::Login}>{"Sign in"}</Link<Route>>
                    {" or "}
                    <Link<Route> to={Route::Register}>{"sign up"}</Link<Route>>
                    {" to add comments on this article."}
                </p>
              }

              {
                for comments.iter().map(|comment| html!{
                    <CommentCard comment={comment.clone()} on_delete={on_delete_comment.clone()} />
                })
              }
            </div>
          </div>
        </div>
      </div>
    }
}

#[derive(PartialEq, Properties)]
struct FollowButtonProps {
    article: UseStatePtrEqHandle<Option<crate::api::Article>>,
}

#[function_component]
fn FollowButton(props: &FollowButtonProps) -> Html {
    let FollowButtonProps { article } = props;

    let auth = use_context::<crate::auth::AuthContext>().unwrap();
    let navigator = use_navigator().unwrap();

    let following = use_state_eq(|| false);

    following.set(article.as_ref().map_or(false, |a| a.author.following));

    let follow = use_async({
        let article = article.clone();
        let navigator = navigator.clone();
        let auth = auth.clone();
        async move {
            if auth.is_unauthorized() || article.is_none() {
                navigator.push(&Route::Register);
                return Ok::<_, Rc<ApiError>>(());
            }

            let url = format!(
                "/api/profiles/{}/follow",
                article.as_ref().unwrap().author.username
            );

            let req = if *following {
                ApiRequest::delete(url)
            } else {
                ApiRequest::post(url)
            };

            let p: UserProfileResp = req.auth(auth.user()).json_response().await?;
            let mut a = article.as_ref().unwrap().clone();
            a.author = p.profile;
            article.set(Some(a));

            Ok::<_, Rc<ApiError>>(())
        }
    });

    let Some(article) = article.as_ref() else {
        return html! {};
    };

    if article.author.following {
        html! {
            <button onclick={move |_| follow.run()} class="btn btn-sm btn-secondary">
                <i class="ion-plus-round"></i>
                {format!("  Unfollow {}", article.author.username)}
            </button>
        }
    } else {
        html! {
            <button onclick={move |_| follow.run()} class="btn btn-sm btn-outline-secondary">
                <i class="ion-plus-round"></i>
                {format!("  Follow {} ", article.author.username)}
            </button>
        }
    }
}

#[derive(PartialEq, Properties)]
struct FavoriteButtonProps {
    article: UseStatePtrEqHandle<Option<crate::api::Article>>,
}

#[function_component]
fn FavoriteButton(props: &FavoriteButtonProps) -> Html {
    let FavoriteButtonProps { article } = props;

    let auth = use_context::<crate::auth::AuthContext>().unwrap();
    let navigator = use_navigator().unwrap();

    let favorited = use_state_eq(|| false);

    favorited.set(article.as_ref().map_or(false, |a| a.favorited));

    let favorite = use_async({
        let article = article.clone();
        let auth = auth.clone();
        async move {
            if auth.is_unauthorized() || article.is_none() {
                navigator.push(&Route::Register);
                return Ok::<_, Rc<ApiError>>(());
            }

            let url = format!("/api/articles/{}/favorite", article.as_ref().unwrap().slug);

            let req = if *favorited {
                ApiRequest::delete(url)
            } else {
                ApiRequest::post(url)
            };

            let a: ArticleResp = req.auth(auth.user()).json_response().await?;
            article.set(Some(a.article));

            Ok::<_, Rc<ApiError>>(())
        }
    });

    let Some(article) = article.as_ref() else {
        return html! {};
    };

    if article.favorited {
        html! {
            <button onclick={move |_| favorite.run() } class="btn btn-sm btn-primary">
                <i class="ion-heart"></i>
                {format!("  Unfavorite Post ")}
                <span class="counter">{format!("({})", article.favorites_count)}</span>
            </button>
        }
    } else {
        html! {
            <button onclick={move |_| favorite.run() } class="btn btn-sm btn-outline-primary">
                <i class="ion-heart"></i>
                {format!("  Favorite Post ")}
                <span class="counter">{format!("({})", article.favorites_count)}</span>
            </button>
        }
    }
}

#[derive(PartialEq, Properties)]
struct EditButtonProps {
    article: UseStatePtrEqHandle<Option<crate::api::Article>>,
}

#[function_component]
fn EditButton(props: &EditButtonProps) -> Html {
    let EditButtonProps { article } = props;

    let slug = article.as_ref().map(|a| a.slug.clone());

    html! {
        <Link<Route> to={Route::Editor{ slug: slug.unwrap() }} classes="btn btn-outline-secondary btn-sm" >
            <i class="ion-edit"></i>{" Edit Article "}
        </Link<Route>>
    }
}

#[derive(PartialEq, Properties)]
struct DeleteButtonProps {
    slug: String,
}

#[function_component]
fn DeleteButton(props: &DeleteButtonProps) -> Html {
    let DeleteButtonProps { slug } = props;

    let auth = use_context::<crate::auth::AuthContext>().unwrap();
    let navigator = use_navigator().unwrap();

    let delete = use_async({
        let auth = auth.clone();
        let slug = slug.clone();

        async move {
            if auth.is_unauthorized() {
                return Ok(());
            }

            let _req: serde_json::Value = ApiRequest::delete(format!("/api/articles/{slug}"))
                .auth(auth.user())
                .json_response()
                .await?;

            navigator.push(&Route::Home);

            Ok::<_, Rc<ApiError>>(())
        }
    });

    html! {
        <button onclick={move |_| delete.run() } class="btn btn-outline-danger btn-sm">
            <i class="ion-trash-a"></i>{" Delete Article "}
        </button>
    }
}

#[derive(PartialEq, Properties)]
pub struct ArticleMetaProps {
    article: UseStatePtrEqHandle<Option<crate::api::Article>>,
}

#[function_component]
pub fn ArticleMeta(props: &ArticleMetaProps) -> Html {
    let ArticleMetaProps {
        article: article_state,
    } = props;

    let auth = use_context::<crate::auth::AuthContext>().unwrap();

    let Some(article) = article_state.as_ref() else {
        return html! {};
    };

    let date = DateTime::<Local>::from(article.created_at).format("%B %e, %Y");

    let my_article = article_state
        .as_ref()
        .map(|article| &article.author.username)
        == auth.user().map(|user| &user.username);

    html! {
        <div class="article-meta">
            <Link<Route> to={Route::Profile {username: article.author.username.clone()}}>
                <img src={article.author.image().to_string()} />
            </Link<Route>>

            <div class="info">
                <Link<Route> to={Route::Profile {username: article.author.username.clone()}} classes="author">
                    {&article.author.username}
                </Link<Route>>
                <span class="date">{date}</span>
            </div>

            if !my_article {
                <FollowButton article={article_state.clone()} />
            } else {
                <EditButton article={article_state.clone()} />
            }
            { "  " }
            if !my_article {
                <FavoriteButton article={article_state.clone()} />
            } else {
                <DeleteButton slug={article_state.as_ref().unwrap().slug.clone()} />
            }
        </div>
    }
}

#[derive(PartialEq, Properties)]
pub struct ArticleBannerProps {
    article: UseStatePtrEqHandle<Option<crate::api::Article>>,
}

#[function_component]
pub fn ArticleBanner(props: &ArticleBannerProps) -> Html {
    let ArticleBannerProps {
        article: article_state,
    } = props;

    let Some(article) = article_state.as_ref() else {
        return html! {};
    };

    html! {
        <>
        <h1>{&article.title}</h1>
        <ArticleMeta article={article_state.clone()}/>
        </>
    }
}

#[derive(PartialEq, Properties)]
pub struct ArticleContentProps {
    article: UseStatePtrEqHandle<Option<crate::api::Article>>,
}

#[function_component]
pub fn ArticleContent(props: &ArticleContentProps) -> Html {
    let ArticleContentProps { article } = props;

    let content_ref = use_node_ref();

    if let Some(article) = article.as_ref() {
        use pulldown_cmark::{html, Parser};
        let parser = Parser::new(&article.body);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);

        let el = content_ref.cast::<Element>().unwrap();
        el.set_inner_html(&html_output);
    }

    html! {
        <>
        <div ref={content_ref}></div>
        if let Some(article) = article.as_ref() {
            <ul class="tag-list">
            {
                for article.tag_list.iter().map(|tag| {
                    html! {
                        <li class="tag-default tag-pill tag-outline">
                            {tag}
                        </li>
                    }
                })
            }
            </ul>
        }
        </>
    }
}

#[derive(PartialEq, Properties)]
pub struct CommentCardProps {
    comment: Comment,
    on_delete: Callback<i32>,
}

#[function_component]
pub fn CommentCard(props: &CommentCardProps) -> Html {
    let CommentCardProps { comment, on_delete } = props;

    let auth = use_context::<crate::auth::AuthContext>().unwrap();

    let date = DateTime::<Local>::from(comment.created_at).format("%B %e, %Y");

    let on_delete = on_delete.clone();
    let comment_id = comment.id;
    let onclick = Callback::from(move |_| on_delete.emit(comment_id));

    html! {
        <div class="card">
            <div class="card-block">
                <p class="card-text">{&comment.body}</p>
            </div>
            <div class="card-footer">
                <Link<Route> to={Route::Profile{ username: comment.author.username.clone() }} classes="comment-author">
                    <img src={comment.author.image().to_string()} class="comment-author-img" />
                </Link<Route>>
                {" "}
                <Link<Route> to={Route::Profile{ username: comment.author.username.clone() }} classes="comment-author">
                    {&comment.author.username}
                </Link<Route>>
                <span class="date-posted">{date}</span>

                if matches!(auth.user(), Some(user) if user.username == comment.author.username) {
                    <span class="mod-options">
                        // <i class="ion-edit"></i>
                        <i {onclick} class="ion-trash-a"></i>
                    </span>
                }
            </div>
        </div>
    }
}
