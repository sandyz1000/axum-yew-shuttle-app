use std::rc::Rc;

use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_hooks::prelude::*;
use yew_router::prelude::*;

use crate::api::{login_user, register_user, ApiError};
use crate::auth::{Auth, AuthContext};
use crate::route::Route;

#[derive(PartialEq, Properties)]
pub struct LoginProps {
    pub mode: LoginMode,
}

#[derive(Clone, Copy, PartialEq)]
pub enum LoginMode {
    SignIn,
    SignUp,
}

#[function_component]
pub fn Login(props: &LoginProps) -> Html {
    let auth = use_context::<AuthContext>().unwrap();

    let mode = props.mode;

    let title = match props.mode {
        LoginMode::SignIn => "Sign in",
        LoginMode::SignUp => "Sign up",
    };

    let other_link = match mode {
        LoginMode::SignIn => html! {
            <Link<Route> to={Route::Register}>{"Need an account?"}</Link<Route>>
        },
        LoginMode::SignUp => html! {
            <Link<Route> to={Route::Login}>{"Have an account?"}</Link<Route>>
        },
    };

    let username_ref = use_node_ref();
    let email_ref = use_node_ref();
    let password_ref = use_node_ref();

    let state = {
        let auth = auth.clone();
        let username_ref = username_ref.clone();
        let email_ref = email_ref.clone();
        let password_ref = password_ref.clone();

        use_async(async move {
            let email = email_ref.cast::<HtmlInputElement>().unwrap().value();
            let password = password_ref.cast::<HtmlInputElement>().unwrap().value();

            match mode {
                LoginMode::SignIn => {
                    let user = login_user(&email, &password).await?;
                    auth.dispatch(Auth::Authorized(user));
                }
                LoginMode::SignUp => {
                    let username = username_ref.cast::<HtmlInputElement>().unwrap().value();

                    let user = register_user(&username, &email, &password).await?;
                    auth.dispatch(Auth::Authorized(user));
                }
            }

            Ok::<_, Rc<ApiError>>(())
        })
    };

    if auth.is_authorized() {
        return html! {
            <Redirect<Route> to={Route::Home}/>
        };
    }

    let onclick = {
        let state = state.clone();
        move |_| state.run()
    };

    let error_message = if let Some(err) = &state.error {
        err.to_vec_string()
    } else {
        vec![]
    };

    html! {
        <div class="auth-page">
            <div class="container page">
                <div class="row">
                    <div class="col-md-6 offset-md-3 col-xs-12">
                        <h1 class="text-xs-center">{title}</h1>

                        <p class="text-xs-center">
                            {other_link}
                        </p>

                        <ul class="error-messages">
                        {
                            for error_message.iter().map(|error_message| {
                                html!{ <li>{error_message}</li> }
                            })
                        }
                        </ul>

                        <form>
                            if props.mode == LoginMode::SignUp {
                                <fieldset class="form-group">
                                    <input ref={username_ref}  disabled={state.loading} class="form-control form-control-lg" type="text" placeholder="Your Name"/>
                                </fieldset>
                            }
                            <fieldset class="form-group">
                                <input ref={email_ref} disabled={state.loading} class="form-control form-control-lg" type="email" placeholder="Email"/>
                            </fieldset>
                            <fieldset class="form-group">
                                <input ref={password_ref} disabled={state.loading} class="form-control form-control-lg" type="password" placeholder="Password"/>
                            </fieldset>
                            <button {onclick} disabled={state.loading} class="btn btn-lg btn-primary pull-xs-right">
                                {title}
                            </button>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    }
}
