use std::rc::Rc;

use serde_json::json;
use yew::prelude::*;
use yew_hooks::{use_async, use_async_with_options, use_state_ptr_eq, UseAsyncOptions};
use yew_router::prelude::*;

use crate::{
    api::{ApiError, ApiRequest, ArticleResp},
    auth::AuthContext,
    route::Route,
};

#[derive(PartialEq)]
struct ArticleData {
    title: String,
    description: String,
    body: String,
    tags: String,
}

#[derive(PartialEq, Properties)]
pub struct EditorProps {
    pub slug: Option<String>,
}

#[function_component]
pub fn Editor(props: &EditorProps) -> Html {
    let EditorProps { slug } = props;

    let auth = use_context::<AuthContext>().unwrap();
    let navigator = use_navigator().unwrap();

    if auth.is_unauthorized() {
        return html! {
            <Redirect<Route> to={Route::Home} />
        };
    }

    let article_data = use_state_ptr_eq(|| None::<ArticleData>);

    let publish = use_async({
        let article_data = article_data.clone();
        let auth = auth.clone();
        let navigator = navigator.clone();
        let slug = slug.clone();
        async move {
            let Some(data) = &*article_data else {
                return Ok(());
            };

            let req = if let Some(slug) = slug {
                ApiRequest::put(format!("/api/articles/{slug}"))
            } else {
                ApiRequest::post("/api/articles")
            };

            let resp: ArticleResp = req
                .auth(auth.user())
                .json(&json!({
                    "article": {
                        "title": &data.title,
                        "description": &data.description,
                        "body": &data.body,
                        "tagList": data.tags.split(",").map(|tag| tag.trim().to_string()).filter(|s| !s.is_empty()).collect::<Vec<_>>(),
                    }
                }))
                .json_response()
                .await?;

            navigator.push(&Route::Article {
                slug: resp.article.slug,
            });

            Ok::<_, Rc<ApiError>>(())
        }
    });

    use_effect_with_deps(
        {
            let publish = publish.clone();
            move |_| publish.run()
        },
        article_data.clone(),
    );

    let error_message = if let Some(err) = &publish.error {
        log::info!("err: {err:?}");
        err.to_vec_string()
    } else {
        vec![]
    };

    html! {
        <div class="editor-page">
            <div class="container page">
                <div class="row">
                    <div class="col-md-10 offset-md-1 col-xs-12">
                        <ul class="error-messages">
                        {
                            for error_message.iter().map(|error_message| {
                                html!{ <li>{error_message}</li> }
                            })
                        }
                        </ul>

                        <EditorForm slug={slug.clone()} on_publish={move |data| article_data.set(Some(data))}/>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[derive(PartialEq, Properties)]
struct EditorFormProps {
    slug: Option<String>,
    on_publish: Callback<ArticleData>,
}

#[function_component]
fn EditorForm(props: &EditorFormProps) -> Html {
    let EditorFormProps { slug, on_publish } = props;

    let article = use_async_with_options(
        {
            let slug = slug.clone();
            async move {
                let slug = slug.ok_or(ApiError::AppError(json!({})))?;
                let resp: ArticleResp = ApiRequest::get(&format!("/api/articles/{slug}"))
                    .json_response()
                    .await?;
                Ok::<_, Rc<ApiError>>(resp.article)
            }
        },
        UseAsyncOptions::enable_auto(),
    );

    let title_ref = use_node_ref();
    let description_ref = use_node_ref();
    let body_ref = use_node_ref();
    let tags_ref = use_node_ref();

    let onclick = {
        let title_ref = title_ref.clone();
        let description_ref = description_ref.clone();
        let body_ref = body_ref.clone();
        let tags_ref = tags_ref.clone();
        let on_publish = on_publish.clone();

        Callback::from(move |_| {
            let title = title_ref
                .cast::<web_sys::HtmlInputElement>()
                .unwrap()
                .value();
            let description = description_ref
                .cast::<web_sys::HtmlInputElement>()
                .unwrap()
                .value();
            let body = body_ref
                .cast::<web_sys::HtmlTextAreaElement>()
                .unwrap()
                .value();
            let tags = tags_ref
                .cast::<web_sys::HtmlInputElement>()
                .unwrap()
                .value();

            on_publish.emit(ArticleData {
                title,
                description,
                body,
                tags,
            });
        })
    };

    html! {
        <form>
            <fieldset>
                <fieldset class="form-group">
                    <input ref={title_ref}
                        type="text"
                        class="form-control form-control-lg"
                        placeholder="Article Title"
                        value={article.data.as_ref().map(|a| a.title.clone())}/>
                </fieldset>
                <fieldset class="form-group">
                    <input ref={description_ref}
                        type="text"
                        class="form-control" placeholder="What's this article about?"
                        value={article.data.as_ref().map(|a| a.description.clone())}/>
                </fieldset>
                <fieldset class="form-group">
                    <textarea ref={body_ref}
                        class="form-control"
                        rows="8"
                        placeholder="Write your article (in markdown)"
                        value={article.data.as_ref().map(|a| a.body.clone())}
                    ></textarea>
                </fieldset>
                <fieldset class="form-group">
                    <input ref={tags_ref}
                        type="text"
                        class="form-control"
                        disabled={slug.is_some()}
                        placeholder="Enter tags"
                        value={article.data.as_ref().map(|a| a.tag_list.join(", "))}/>
                    <div class="tag-list"></div>
                </fieldset>
                <button {onclick} class="btn btn-lg pull-xs-right btn-primary" type="button">
                        {"Publish Article"}
                </button>
            </fieldset>
        </form>
    }
}
