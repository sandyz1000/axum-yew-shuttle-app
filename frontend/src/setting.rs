use std::rc::Rc;

use serde_json::json;
use web_sys::{HtmlInputElement, HtmlTextAreaElement};
use yew::prelude::*;
use yew_hooks::use_async;
use yew_router::prelude::*;

use crate::{
    api::{ApiError, ApiRequest, UserAuthResp},
    auth::{Auth, AuthContext},
    route::Route,
};

#[function_component]
pub fn Setting() -> Html {
    let auth = use_context::<AuthContext>().unwrap();

    let image_ref = use_node_ref();
    let username_ref = use_node_ref();
    let bio_ref = use_node_ref();
    let email_ref = use_node_ref();
    let password_ref = use_node_ref();

    let image = use_state_eq(|| "".to_string());
    let username = use_state_eq(|| "".to_string());
    let bio = use_state_eq(|| "".to_string());
    let email = use_state_eq(|| "".to_string());

    let auth = auth.clone();
    let email_ref = email_ref.clone();
    let username_ref = username_ref.clone();
    let password_ref = password_ref.clone();
    let bio_ref = bio_ref.clone();
    let image_ref = image_ref.clone();

    let update = use_async({
        let auth = auth.clone();
        let email_ref = email_ref.clone();
        let username_ref = username_ref.clone();
        let password_ref = password_ref.clone();
        let bio_ref = bio_ref.clone();
        let image_ref = image_ref.clone();

        async move {
            let user: UserAuthResp = ApiRequest::put("/api/user")
                .auth(auth.user())
                .json(&json!({
                    "user": {
                        "email": email_ref.cast::<HtmlInputElement>().unwrap().value(),
                        "username": username_ref.cast::<HtmlInputElement>().unwrap().value(),
                        "password": password_ref.cast::<HtmlInputElement>().unwrap().value(),
                        "bio": bio_ref.cast::<HtmlTextAreaElement>().unwrap().value(),
                        "image": image_ref.cast::<HtmlInputElement>().unwrap().value(),
                    }
                }))
                .json_response()
                .await?;

            auth.dispatch(Auth::Authorized(user.user));

            Ok::<_, Rc<ApiError>>(())
        }
    });

    if auth.is_unauthorized() {
        return html! {
            <Redirect<Route> to={Route::Home} />
        };
    }

    if let Some(auth) = auth.user() {
        if let Some(r) = &auth.image {
            image.set(r.clone());
        }
        username.set(auth.username.clone());
        if let Some(r) = &auth.bio {
            bio.set(r.clone());
        }
        email.set(auth.email.clone());
    }

    let onclick_update = {
        let update = update.clone();
        move |_| update.run()
    };

    let onclick_logout = {
        let auth = auth.clone();
        Callback::from(move |_| {
            auth.dispatch(Auth::Unauthorized);
        })
    };

    let setting_form = html! {
        <fieldset>
            <fieldset class="form-group">
                <input
                    ref={image_ref}
                    class="form-control"
                    type="text"
                    placeholder="URL of profile picture"
                    value={(*image).clone()}
                    disabled={update.loading}
                />
            </fieldset>

            <fieldset class="form-group">
                <input
                    ref={username_ref}
                    class="form-control form-control-lg"
                    type="text"
                    placeholder="Your Name"
                    value={(*username).clone()}
                    disabled={update.loading}
                />
            </fieldset>

            <fieldset class="form-group">
                <textarea
                    ref={bio_ref}
                    class="form-control form-control-lg"
                    rows="8"
                    placeholder="Short bio about you"
                    value={(*bio).clone()}
                    disabled={update.loading}
                ></textarea>
            </fieldset>

            <fieldset class="form-group">
                <input
                    ref={email_ref}
                    class="form-control form-control-lg"
                    type="text"
                    placeholder="Email"
                    value={(*email).clone()}
                    disabled={update.loading}
                />
            </fieldset>

            <fieldset class="form-group">
                <input ref={password_ref}
                    class="form-control form-control-lg"
                    type="password"
                    placeholder="New Password"
                    disabled={update.loading} />
            </fieldset>

            <button
                class="btn btn-lg btn-primary pull-xs-right"
                onclick={onclick_update}
                disabled={update.loading}
            >{"Update Settings"}</button>
    </fieldset>

    };

    html! {
        <div class="settings-page">
            <div class="container page">
                <div class="row">
                    <div class="col-md-6 offset-md-3 col-xs-12">
                        <h1 class="text-xs-center">{"Your Settings"}</h1>

                        <form>
                            {setting_form}
                        </form>
                        <hr />
                        <button class="btn btn-outline-danger" onclick={onclick_logout}>
                            {"Or click here to logout."}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    }
}
