pub mod error_template;

#[cfg(feature = "ssr")]
#[path = ""]
mod ssr_modules {
    pub mod auth;
    pub mod fileserv;
    pub mod middlewares;

    pub(super) use middlewares::{auth_role, require_login};
}
#[cfg(feature = "ssr")]
pub use ssr_modules::*;

use error_template::{AppError, ErrorTemplate};
use leptos::*;
use leptos_meta::*;
use leptos_router::*;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos_dom::HydrationCtx::stop_hydrating();
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/auth-middleware.css"/>
        <Script src="/pkg/uikit-core.min.js"></Script>
        <Title text="Welcome to Leptos"/>
        <Router fallback=|| {
            let mut outside_errors = Errors::default();
            outside_errors.insert_with_default_key(AppError::NotFound);
            view! { <ErrorTemplate outside_errors/> }.into_view()
        }>
            <main>
                <h1>"Welcome to Leptos!"</h1>
                <h1 class="title-auth">"Auth middleware"</h1>
                <Routes>
                    <Route path="" view=HomePage/>
                    <Route path="/protected" view=Authenticated/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    view! {
        <Login/>
    }
}

#[component]
fn Authenticated() -> impl IntoView {
    view! {
        <p>"Sensitive Data"</p>
        <ProtectedOcean>
            <ProtectedData/>
            <Logout/>
        </ProtectedOcean>
    }
}

#[derive(Clone)]
struct LoggedIn(pub bool);

#[island]
fn ProtectedOcean(children: Children) -> impl IntoView {
    let session_check = create_server_action::<CheckSession>();
    let logged_in = create_local_resource(move || session_check.value().get(), |_| check_session());
    let (auth, set_auth) = create_signal(LoggedIn(false));
    provide_context(auth);

    view! {
        <Suspense fallback= || ()>
            {move || if logged_in.get().is_some_and(|v| v.is_ok()) {
                set_auth(LoggedIn(true));
            } else {
                set_auth(LoggedIn(false));
            }}
        </Suspense>
        {children()}
    }
}

#[island]
fn ProtectedData() -> impl IntoView {
    let super_secret_action = create_server_action::<FetchSecretData>();
    let fetch_data_action = create_server_action::<FetchData>();
    let data_fetched =
        create_local_resource(move || fetch_data_action.value().get(), |_| fetch_data());
    let auth = expect_context::<ReadSignal<LoggedIn>>();

    view! {
        <div style:display=move || if auth().0 { "block"} else { "none"}>
            <Suspense fallback=|| view!{
                    <div uk-spinner="ratio: 4"></div>
                }>
                <div class="uk-flex uk-flex-column uk-flex-middle">
                    <p class="sensitive-data">
                        {data_fetched}
                    </p>

                    <Show when=move || super_secret_action.value().get().is_some_and(|v| v.is_ok())>
                        <p class="sensitive-data">
                            {super_secret_action.value().get().unwrap().unwrap()}
                        </p>
                    </Show>
                </div>
            </Suspense>
            <ActionForm action=super_secret_action>
                <button class="uk-button uk-button-secondary" type="submit">
                    "Request Data"
                </button>
            </ActionForm>
        <br/>
        <hr class="uk-divider-small"/>
        </div>
    }
}

#[island]
fn Login() -> impl IntoView {
    let login_action = create_server_action::<LoginSFn>();
    let session_check = create_server_action::<CheckSession>();
    let logged_in = create_local_resource(move || session_check.value().get(), |_| check_session());

    view! {
        <Suspense fallback= move || view!{
                <div uk-spinner="ratio: 1"></div>
            }>
            {move || if logged_in.get().is_some_and(|v| v.is_ok()) {
                view! {
                    <>
                    <div uk-spinner></div>
                    {window().location().set_href("/protected").unwrap()}
                    </>
                }
            } else {
                view!{
                    <>
                    <ActionForm action=login_action>
                        <button class="button-auth uk-button-large" type="submit">
                            "Login"
                        </button>
                    </ActionForm>
                    </>
                }
            }}
        </Suspense>
    }
}

#[island]
fn Logout() -> impl IntoView {
    let logout_action = create_server_action::<LogoutSFn>();
    let logged_in = expect_context::<ReadSignal<LoggedIn>>();

    view! {
        <div style:display=move || if logged_in().0 { "block"} else { "none"}>
        <ActionForm action=logout_action>
            <button class="button-auth uk-button-small" type="submit">
                "Logout"
            </button>
        </ActionForm>
        </div>
    }
}

#[server(LoginSFn)]
async fn login() -> Result<(), ServerFnError> {
    use auth::{AuthSession, UserId};
    use axum::Extension;

    let res = expect_context::<leptos_axum::ResponseOptions>();
    let Extension(mut auth_session) = leptos_axum::extract::<Extension<AuthSession>>().await?;
    let user = match auth_session
        .authenticate(UserId("leptos_user".into()))
        .await
    {
        Ok(Some(user)) => {
            leptos_axum::redirect("/protected");
            user
        }
        Ok(None) => {
            res.set_status(http::StatusCode::UNAUTHORIZED);
            return Err(ServerFnError::new(""));
        }
        Err(_) => {
            res.set_status(http::StatusCode::INTERNAL_SERVER_ERROR);
            return Err(ServerFnError::ServerError("".to_string()));
        }
    };

    if auth_session.login(&user).await.is_err() {
        res.set_status(http::StatusCode::INTERNAL_SERVER_ERROR);
        return Err(ServerFnError::ServerError("".to_string()));
    }

    Ok(())
}

#[server(LogoutSFn)]
async fn logout() -> Result<(), ServerFnError> {
    use auth::AuthSession;
    use axum::Extension;

    let Extension(mut auth_session) = leptos_axum::extract::<Extension<AuthSession>>().await?;
    let res = expect_context::<leptos_axum::ResponseOptions>();
    leptos_axum::redirect("/");
    match auth_session.logout().await {
        Ok(_) => Ok(()),
        Err(err) => {
            res.set_status(http::StatusCode::INTERNAL_SERVER_ERROR);
            Err(ServerFnError::ServerError(err.to_string()))
        }
    }
}

#[server(CheckSession)]
#[middleware(compose_from_fn!(require_login))]
async fn check_session() -> Result<(), ServerFnError> {
    Ok(())
}

#[server(FetchData)]
#[middleware(compose_from_fn!(require_login, |req| auth_role(req, auth::Role::User)))]
async fn fetch_data() -> Result<String, ServerFnError> {
    use auth::AuthSession;
    use axum::Extension;
    use leptos_axum::extract;

    let Extension(session) = extract::<Extension<AuthSession>>().await?;
    println!("Session id {0:?}", session.user);

    let sensitive_information =
        "Failure is not an Option<T>, it's a Result<T,E> \nYou're a member!";
    Ok(sensitive_information.to_string())
}

#[server(FetchSecretData)]
#[middleware(compose_from_fn!(require_login, |req| auth_role(req, auth::Role::Admin) ))]
async fn super_secret_data() -> Result<String, ServerFnError> {
    use auth::UserId;
    use axum::Extension;
    use leptos_axum::extract;

    let Extension(user_id) = extract::<Extension<UserId>>().await?;
    println!("UserId: {:?}", user_id);

    let secret_data = "You're an admin!";
    Ok(secret_data.to_string())
}
