use std::rc::Rc;

use yew::prelude::*;
use yew_hooks::prelude::*;
use yew_router::prelude::use_navigator;

use crate::{
    api::{ApiError, ApiRequest, UserProfile, UserProfileResp},
    feed::{Feed, FeedTab, FeedType, Tab},
    route::Route,
};

#[derive(PartialEq, Properties)]
pub struct ProfileProps {
    pub username: String,
}

#[function_component]
pub fn Profile(props: &ProfileProps) -> Html {
    let ProfileProps { username } = props;

    let auth = use_context::<crate::auth::AuthContext>().unwrap();

    let profile = use_state_ptr_eq(|| None);

    let reload_profile = {
        let username = username.clone();
        let profile = profile.clone();
        let auth = auth.clone();
        use_async(async move {
            let p: UserProfileResp = ApiRequest::get(&format!("/api/profiles/{username}"))
                .auth(auth.user())
                .json_response()
                .await?;
            profile.set(Some(p.profile));
            Ok::<_, Rc<ApiError>>(())
        })
    };

    let cur_tab = use_state(|| FeedType::User(username.clone()));

    use_effect_with_deps(
        {
            let reload_profile = reload_profile.clone();
            let cur_tab = cur_tab.clone();
            let username = username.clone();
            move |_| {
                cur_tab.set(FeedType::User(username));
                reload_profile.run();
            }
        },
        (username.clone(), auth.clone()),
    );

    let tabs = vec![
        Tab {
            name: "My Articles".to_string(),
            value: FeedType::User(username.clone()),
        },
        Tab {
            name: "Favorited Articles".to_string(),
            value: FeedType::Favorited(username.clone()),
        },
    ];

    html! {
        <div class="profile-page">
            <div class="user-info">
                <div class="container">
                    <div class="row">
                        <ProfileHeader profile={profile.clone()} />
                    </div>
                </div>
            </div>

            <div class="container">
                <div class="row">
                    <div class="col-xs-12 col-md-10 offset-md-1">
                        <div class="articles-toggle">
                            <FeedTab {tabs} cur_tab={(*cur_tab).clone()}
                                onclick={let cur_tab = cur_tab.clone(); move |tab| cur_tab.set(tab)} />
                        </div>
                        <Feed feed_type={(*cur_tab).clone()} limit=5 />
                    </div>
                </div>
            </div>
      </div>
    }
}

#[derive(PartialEq, Properties)]
struct ProfileHeaderProps {
    profile: UseStatePtrEqHandle<Option<UserProfile>>,
}

#[function_component]
fn ProfileHeader(props: &ProfileHeaderProps) -> Html {
    let ProfileHeaderProps { profile } = props;
    let profile = profile.clone();

    let auth = use_context::<crate::auth::AuthContext>().unwrap();
    let navigator = use_navigator().unwrap();

    let image = profile
        .as_ref()
        .map(|p| p.image())
        .unwrap_or("")
        .to_string();

    let username = profile.as_ref().map_or("", |p| &p.username).to_string();

    let bio = profile
        .as_ref()
        .and_then(|p| p.bio.as_deref())
        .unwrap_or("")
        .to_string();

    let following = use_state_eq(|| false);

    following.set(profile.as_ref().map_or(false, |p| p.following));

    let follow = use_async({
        let auth = auth.clone();
        let navigator = navigator.clone();
        async move {
            if let Some(p) = profile.as_ref() {
                if auth.is_authorized() {
                    let req = if p.following {
                        ApiRequest::delete(format!("/api/profiles/{}/follow", p.username))
                    } else {
                        ApiRequest::post(format!("/api/profiles/{}/follow", p.username))
                    };

                    let prof: UserProfileResp = req.auth(auth.user()).json_response().await?;
                    profile.set(Some(prof.profile));
                } else {
                    navigator.push(&Route::Register);
                }
            }

            Ok::<_, Rc<ApiError>>(())
        }
    });

    html! {
        <div class="col-xs-12 col-md-10 offset-md-1">
            <img src={image} class="user-img" />
            <h4>{&username}</h4>
            <p>{bio}</p>
            if auth.user().map_or(false, |u| u.username == username) {
                <button onclick={ move |_| navigator.push(&Route::Setting) }
                    class="btn btn-sm btn-outline-secondary action-btn">
                    <i class="ion-gear-a
                    "></i>
                    { "  Edit Profile Settings" }
                </button>
            } else if *following {
                <button onclick={ move |_| follow.run() } class="btn btn-sm btn-secondary action-btn">
                    <i class="ion-plus-round"></i>
                    { format!("  Unfollow {username}") }
                </button>
            } else {
                <button onclick={ move |_| follow.run() } class="btn btn-sm btn-outline-secondary action-btn">
                    <i class="ion-plus-round"></i>
                    { format!("  Follow {username}") }
                </button>
            }
        </div>
    }
}
