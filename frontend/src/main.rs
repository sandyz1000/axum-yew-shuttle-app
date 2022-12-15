mod api;
mod article;
mod auth;
mod editor;
mod feed;
mod home;
mod login;
mod profile;
mod route;
mod setting;

use yew::prelude::*;
use yew_router::prelude::*;

use crate::{
    auth::{AuthContext, AuthProvider},
    route::Route,
};

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}

#[function_component]
fn App() -> Html {
    html! {
        <AuthProvider>
            <HashRouter>
                <Switch<Route> render={switch}/>
            </HashRouter>
        </AuthProvider>
    }
}

fn switch(routes: Route) -> Html {
    let content = match &routes {
        Route::Home => html! { <home::Home /> },
        Route::Login => html! { <login::Login mode={login::LoginMode::SignIn} /> },
        Route::Register => html! { <login::Login mode={login::LoginMode::SignUp} /> },
        Route::Setting => html! { <setting::Setting /> },
        Route::NewArticle => html! { <editor::Editor slug={None::<String>}/> },
        Route::Editor { slug } => html! { <editor::Editor slug={Some(slug.clone())} /> },
        Route::Article { slug } => html! { <article::Article slug={slug.clone()} /> },
        Route::Profile { username } => html! { <profile::Profile username={username.clone()} /> },
        Route::NotFound => html! { <Redirect<Route> to={Route::Home} /> },
    };

    html! {
        <>
            <Header route={routes} />
            {content}
            <Footer />
        </>
    }
}

#[derive(PartialEq, Properties)]
struct HeaderProps {
    route: Route,
}

#[function_component]
fn Header(props: &HeaderProps) -> Html {
    let HeaderProps { route } = props;

    let auth = use_context::<AuthContext>().unwrap();

    html! {
        <nav class="navbar navbar-light">
            <div class="container">
                <Link<Route> classes="navbar-brand" to={Route::Home}>{"conduit"}</Link<Route>>

                <ul class="nav navbar-nav pull-xs-right">
                    <HeaderLink route={route.clone()} to={Route::Home}>
                        {"Home"}
                    </HeaderLink>

                    if let Some(user) = auth.user() {
                        <HeaderLink route={route.clone()} to={Route::NewArticle}>
                            <i class="ion-compose"></i>
                            {" New Article"}
                            </HeaderLink>
                        <HeaderLink route={route.clone()} to={Route::Setting}>
                            <i class="ion-gear-a"></i>
                            {" Settings"}
                        </HeaderLink>
                        <HeaderLink route={route.clone()} to={Route::Profile { username: user.username.clone() }}>
                            <img class="user-pic"
                                src={user.image().to_string()}/>
                            {&user.username}
                        </HeaderLink>
                    }

                    if auth.is_unauthorized() {
                        <HeaderLink route={route.clone()} to={Route::Login}>
                            {"Sign in"}
                        </HeaderLink>
                        <HeaderLink route={route.clone()} to={Route::Register}>
                            {"Sign up"}
                        </HeaderLink>
                    }
                </ul>
            </div>
        </nav>
    }
}

#[derive(PartialEq, Properties)]
struct HeaderLinkProps {
    route: Route,
    to: Route,
    children: Children,
}

#[function_component]
fn HeaderLink(props: &HeaderLinkProps) -> Html {
    let HeaderLinkProps {
        route,
        to,
        children,
    } = props;

    let active = if route == to { Some("active") } else { None };

    html! {
        <li class="nav-item">
            <Link<Route> classes={classes!("nav-link", active)} to={to.clone()}>
                { for children.iter() }
            </Link<Route>>
        </li>
    }
}

#[function_component]
fn Footer() -> Html {
    html! {
        <footer>
            <div class="container">
                <a href="/" class="logo-font">{"conduit"}</a>
                <span class="attribution">
                    {"An interactive learning project from "}
                    <a href="https://thinkster.io">{"Thinkster"}</a>
                    {". Code & design licensed under MIT."}
                </span>
            </div>
        </footer>
    }
}
