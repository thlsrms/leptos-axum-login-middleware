#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use auth_middleware::fileserv::file_and_error_handler;
    use axum::Router;
    use leptos::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};

    // Setting get_configuration(None) means we'll be using cargo-leptos's env values
    // For deployment these variables are:
    // <https://github.com/leptos-rs/start-axum#executing-a-server-on-a-remote-machine-without-the-toolchain>
    // Alternately a file can be specified such as Some("Cargo.toml")
    // The file would need to be included with the executable when moved to deployment
    let conf = get_configuration(None).await.unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(auth_middleware::App);

    use auth_middleware::auth;
    use axum_login::tower_sessions::{MemoryStore, SessionManagerLayer};
    use axum_login::AuthManagerLayerBuilder;

    let mut auth_backend = auth::Backend::default();
    // roles: Admin = 255 and User = 100
    let _ = auth_backend.register_user("leptos_user", &[255]);

    let auth_layer = AuthManagerLayerBuilder::new(
        auth_backend,
        SessionManagerLayer::new(MemoryStore::default()),
    )
    .build();

    // build our application with a route
    let app = Router::new()
        .leptos_routes(&leptos_options, routes, auth_middleware::App)
        .layer(auth_layer)
        .fallback(file_and_error_handler)
        .with_state(leptos_options);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for a purely client-side app
    // see lib.rs for hydration function instead
}
