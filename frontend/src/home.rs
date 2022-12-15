use std::rc::Rc;

use yew::prelude::*;
use yew_hooks::{use_async_with_options, UseAsyncOptions};

use crate::{
    api::{ApiError, TagsResp},
    auth::AuthContext,
    feed::{Feed, FeedTab, FeedType, Tab},
};

#[function_component]
pub fn Home() -> Html {
    let auth = use_context::<AuthContext>().unwrap();

    let cur_tab = use_state(|| FeedType::Global);

    use_effect_with_deps(
        {
            let feed_type = cur_tab.clone();
            let auth = auth.clone();
            move |_| {
                if auth.is_authorized() {
                    feed_type.set(FeedType::UserFeed);
                }
            }
        },
        auth.is_loading(),
    );

    let mut tabs = vec![];

    if auth.is_authorized() {
        tabs.push(Tab {
            name: "Your Feed".to_string(),
            value: FeedType::UserFeed,
        });
    }

    tabs.push(Tab {
        name: "Global Feed".to_string(),
        value: FeedType::Global,
    });

    if let FeedType::Tag(tag) = &*cur_tab {
        tabs.push(Tab {
            name: format!("#{}", tag),
            value: FeedType::Tag(tag.clone()),
        });
    }

    let onclick_tab = {
        let cur_tab = cur_tab.clone();
        move |tab| {
            cur_tab.set(tab);
        }
    };

    let onclick_tag = {
        let feed_type = cur_tab.clone();
        move |tag| {
            feed_type.set(FeedType::Tag(tag));
        }
    };

    html! {
        <div class="home-page">

        if auth.is_unauthorized() {
            <div class="banner">
                <div class="container">
                    <h1 class="logo-font">{"conduit"}</h1>
                    <p>{"A place to share your knowledge."}</p>
                </div>
            </div>
        }

        <div class="container page">
            <div class="row">
                <div class="col-md-9">
                    <div class="feed-toggle">
                        <FeedTab {tabs} cur_tab={(*cur_tab).clone()} onclick={onclick_tab} />
                    </div>

                    <Feed feed_type={(*cur_tab).clone()} limit=10 />
                </div>

                <div class="col-md-3">
                    <div class="sidebar">
                        <p>{"Popular Tags"}</p>
                        <Tags onclick={onclick_tag} />
                    </div>
                </div>

            </div>
        </div>
    </div>
    }
}

#[derive(PartialEq, Properties)]
struct TagsProps {
    onclick: Callback<String>,
}

#[function_component]
fn Tags(props: &TagsProps) -> Html {
    let TagsProps { onclick } = props;

    let tags = use_state(|| vec![]);

    use_async_with_options(
        {
            let tags = tags.clone();
            async move {
                let t: TagsResp = crate::api::ApiRequest::get("/api/tags")
                    .json_response()
                    .await?;
                tags.set(t.tags);
                Ok::<_, Rc<ApiError>>(())
            }
        },
        UseAsyncOptions::enable_auto(),
    );

    html! {
        <div class="tag-list">
        {
            for tags.iter().cloned().map(|tag| {
                html! {
                    <a onclick={
                        let onclick = onclick.clone();
                        let tag = tag.clone();
                        move |_| onclick.emit(tag.clone())
                    } href="javascript:void(0);" class="tag-pill tag-default"
                    >{tag}</a>
                }
            })
        }
        </div>
    }
}
