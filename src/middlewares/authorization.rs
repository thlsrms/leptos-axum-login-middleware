use std::task::{Context, Poll};

use axum::body::Body;
use axum_login::{AuthUser, AuthzBackend};
use http::{Request, Response, StatusCode};
use tower::{Layer, Service};

use crate::auth::{AuthSession, Role, UserId};

use super::future::{ResponseFuture, State};
use super::require_login::RequireLoginService;

pub struct AuthorizationLayer {
    permission_required: Role,
    login_uri: String,
}

impl AuthorizationLayer {
    pub fn new(permission_required: Role, login_uri: &str) -> Self {
        Self {
            permission_required,
            login_uri: login_uri.into(),
        }
    }
}

impl<S> Layer<S> for AuthorizationLayer {
    type Service = RequireLoginService<AuthorizationService<S>>;

    fn layer(&self, inner: S) -> Self::Service {
        RequireLoginService {
            login_uri: self.login_uri.clone(),
            inner: AuthorizationService {
                inner,
                permission_required: self.permission_required,
            },
        }
    }
}

#[derive(Clone)]
pub struct AuthorizationService<S> {
    inner: S,
    permission_required: Role,
}

impl<S> Service<Request<Body>> for AuthorizationService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = ResponseFuture<S>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        let permission_required = self.permission_required;

        let future = Box::pin(async move {
            let Some(auth_session) = req.extensions().get::<AuthSession>() else {
                return Err(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .unwrap());
            };

            let is_authorized: Option<UserId> = {
                if let Some(user) = &auth_session.user {
                    if auth_session
                        .backend
                        .has_perm(user, permission_required.into())
                        .await
                        .unwrap_or(false)
                    {
                        Some(UserId(user.id()))
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
                .insert::<UserId>(is_authorized.unwrap());

            Ok(req)
        });

        ResponseFuture {
            state: State::Pending { future },
            inner: self.inner.clone(),
        }
    }
}
