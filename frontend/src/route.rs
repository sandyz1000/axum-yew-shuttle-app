use yew_router::prelude::*;

#[derive(Debug, Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/login")]
    Login,
    #[at("/register")]
    Register,
    #[at("/setting")]
    Setting,
    #[at("/editor")]
    NewArticle,
    #[at("/editor/:slug")]
    Editor { slug: String },
    #[at("/article/:slug")]
    Article { slug: String },
    #[at("/profile/:username")]
    Profile { username: String },
    #[not_found]
    #[at("/404")]
    NotFound,
}
