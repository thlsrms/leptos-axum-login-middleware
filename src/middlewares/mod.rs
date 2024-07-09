mod macros;

pub use macros::MiddlewareLayer;

use super::auth;

use axum::body::Body;
use axum_login::{AuthUser, AuthzBackend};
use http::{Request, Response, StatusCode};

pub async fn require_login(req: Request<Body>) -> Result<Request<Body>, Response<Body>> {
    let Some(auth_session) = req.extensions().get::<auth::AuthSession>() else {
        return Err(Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::empty())
            .unwrap());
    };

    if auth_session.user.is_none() {
        let headers = req.headers();
        let base_url: Option<&str> =
            headers
                .get(http::header::REFERER)
                .and_then(|referer| match referer.to_str() {
                    Ok(referer_str) => {
                        headers
                            .get(http::header::ORIGIN)
                            .and_then(|origin| match origin.to_str() {
                                Ok(origin_str) => referer_str.strip_prefix(origin_str),
                                Err(_) => None,
                            })
                    }
                    Err(_) => None,
                });

        if let Some(base_url) = base_url {
            // Redirect if the request did not originated from the login page
            if base_url != "/" {
                leptos_axum::redirect("/");
            }
        }

        return Err(Response::builder().body(Body::empty()).unwrap());
    }

    Ok(req)
}

pub async fn auth_role(
    mut req: Request<Body>,
    role: auth::Role,
) -> Result<Request<Body>, Response<Body>> {
    let Some(auth_session) = req.extensions().get::<auth::AuthSession>() else {
        return Err(Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::empty())
            .unwrap());
    };

    let is_authorized = {
        if let Some(user) = &auth_session.user {
            if auth_session
                .backend
                .has_perm(user, role.into())
                .await
                .unwrap_or(false)
            {
                Some(auth::UserId(user.id()))
            } else {
                None
            }
        } else {
            None
        }
    };

    if is_authorized.is_none() {
        return Err(Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Body::empty())
            .unwrap());
    }

    req.extensions_mut()
        .insert::<auth::UserId>(is_authorized.unwrap());

    Ok(req)
}
