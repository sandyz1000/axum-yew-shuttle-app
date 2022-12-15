use std::rc::Rc;

use chrono::{DateTime, Local};
use yew::prelude::*;
use yew_hooks::prelude::*;
use yew_router::prelude::*;

use crate::{
    api::{ApiError, ApiRequest, Article, ArticleResp, MultipleArticle},
    auth::AuthContext,
    route::Route,
};

#[derive(PartialEq, Properties)]
pub struct FeedTabProps {
    pub tabs: Vec<Tab>,
    pub cur_tab: FeedType,
    pub onclick: Callback<FeedType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    pub name: String,
    pub value: FeedType,
}

#[function_component]
pub fn FeedTab(props: &FeedTabProps) -> Html {
    let FeedTabProps {
        tabs,
        cur_tab,
        onclick,
    } = props;

    html! {
        <ul class="nav nav-pills outline-active">
        {
            for tabs.iter().cloned().map(|tab| {
                let onclick = onclick.clone();
                html! {
                <li class="nav-item">
                    <a class={classes!("nav-link", if &tab.value == cur_tab {Some("active")} else {None})}
                        onclick={ move |_| onclick.emit(tab.value.clone()) }
                        href="javascript:void(0);"
                    >{&tab.name}</a>
                </li>
                }
            })
        }
        </ul>
    }
}

#[derive(PartialEq, Properties)]
pub struct FeedProps {
    pub limit: usize,
    pub feed_type: FeedType,
}

#[derive(Debug, PartialEq, Clone)]
pub enum FeedType {
    Global,
    UserFeed,
    Tag(String),
    User(String),
    Favorited(String),
}

#[function_component]
pub fn Feed(props: &FeedProps) -> Html {
    let FeedProps { limit, feed_type } = props;

    let auth = use_context::<AuthContext>().unwrap();
    let navigator = use_navigator().unwrap();

    let cur_page = use_state_eq(|| 0);

    use_effect_with_deps(
        {
            let cur_page = cur_page.clone();
            move |_| cur_page.set(0)
        },
        feed_type.clone(),
    );

    let feed = {
        let auth = auth.clone();
        let feed_type = feed_type.clone();
        let limit = limit.clone();
        let cur_page = cur_page.clone();

        use_async(async move {
            let url = match feed_type {
                FeedType::Global => "/api/articles".to_string(),
                FeedType::UserFeed => "/api/articles/feed".to_string(),
                FeedType::Tag(tag) => format!("/api/articles?tag={tag}"),
                FeedType::User(username) => format!("/api/articles?author={username}"),
                FeedType::Favorited(username) => format!("/api/articles?favorited={username}"),
            };

            let articles: MultipleArticle = ApiRequest::get(&url)
                .query([("limit", limit.to_string())])
                .query([("offset", (*cur_page * limit).to_string())])
                .auth(auth.user())
                .json_response()
                .await?;

            Ok::<_, Rc<ApiError>>(Rc::new(articles))
        })
    };

    let update_feed = use_bool_toggle(false);

    use_effect_with_deps(
        {
            let feed = feed.clone();
            move |_| {
                feed.run();
                || {}
            }
        },
        ((*feed_type).clone(), *update_feed, *cur_page),
    );

    let fav_arg = use_state(|| None);

    let send_fav = use_async({
        let auth = auth.clone();
        let fav_arg = fav_arg.clone();
        let update_feed = update_feed.clone();

        async move {
            let Some((slug, fav)) = &*fav_arg else {
                return Ok::<_, Rc<ApiError>>(())
            };

            let url = format!("/api/articles/{slug}/favorite");

            let req = if *fav {
                ApiRequest::post(&url)
            } else {
                ApiRequest::delete(&url)
            };

            let _: ArticleResp = req.auth(auth.user()).json_response().await?;

            update_feed.toggle();

            Ok(())
        }
    });

    let fav_callback = Rc::new(Callback::from({
        let auth = auth.clone();
        move |(slug, fav)| {
            if auth.is_unauthorized() {
                navigator.push(&Route::Register);
            } else {
                fav_arg.set(Some((slug, fav)));
                send_fav.run();
            }
        }
    }));

    let Some(articles) = feed.data.as_ref() else {
        return html! { <div class="article-preview">{"Loading articles..."}</div> };
    };

    if articles.articles.is_empty() {
        return html! { <div class="article-preview">{"No articles are here... yet."}</div> };
    }

    let pages = (articles.articles_count + limit - 1) / limit;

    html! {
        <>
        {
            for articles.articles.iter().map(|article| html! {
                <ArticleCard article={article.clone()} fav_callback={fav_callback.clone()} />
            })
        }
        if pages >= 2 {
            <nav>
                <ul class="pagination">
                {
                    for (0..pages).map(|page| {
                        html!{
                            <li class={classes!("page-item", if page == *cur_page {Some("active")} else {None})}>
                                <a class="page-link ng-binding" href="javascript:void(0);"
                                    onclick={ let cur_page = cur_page.clone(); move |_| cur_page.set(page) }>
                                    {page + 1}
                                </a>
                            </li>
                        }
                    })
                }
                </ul>
            </nav>
        }
        </>
    }
}

#[derive(PartialEq, Properties)]
pub struct ArticleCardProps {
    article: Article,
    fav_callback: Rc<Callback<(String, bool)>>,
}

#[function_component]
pub fn ArticleCard(props: &ArticleCardProps) -> Html {
    let ArticleCardProps {
        article,
        fav_callback,
    } = props;

    let date = DateTime::<Local>::from(article.created_at).format("%B %e, %Y");
    let btn_outline = if article.favorited {
        "btn-primary"
    } else {
        "btn-outline-primary"
    };

    let onclick = {
        let fav_callback = fav_callback.clone();
        let slug = article.slug.clone();
        let fav = article.favorited;
        move |_| {
            fav_callback.emit((slug.clone(), !fav));
        }
    };

    html! {
        <div class="article-preview">
            <div class="article-meta">
                <Link<Route> to={Route::Profile{ username: article.author.username.clone() }}>
                    <img src={article.author.image().to_string()}/>
                </Link<Route>>
                <div class="info">
                    <Link<Route> to={Route::Profile{ username: article.author.username.clone() }} classes="author">
                        {&article.author.username}
                    </Link<Route>>
                    <span class="date">{date}</span>
                </div>
                <button {onclick} class={classes!("btn", "btn-sm", "pull-xs-right", btn_outline)}>
                    <i class="ion-heart"></i>{" "}{article.favorites_count}
                </button>
            </div>
            <Link<Route> to={Route::Article { slug: article.slug.clone() }} classes="preview-link">
                <h1>{&article.title}</h1>
                <p>{&article.description}</p>
                <span>{"Read more..."}</span>

                <ul class="tag-list">
                    { for article.tag_list.iter().map(|tag| html! {
                        <li class="tag-default tag-pill tag-outline">
                            {tag}
                        </li>
                    })}
                </ul>
            </Link<Route>>
        </div>
    }
}
