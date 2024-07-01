use std::task::{Context, Poll};

use axum::body::Body;
use http::{Request, Response, StatusCode};
use tower::{Layer, Service};

use crate::auth::AuthSession;

use super::future::{ResponseFuture, State};

#[derive(Default)]
pub struct RequireLoginLayer {
    login_uri: String,
}

impl RequireLoginLayer {
    pub fn new(login_uri: &str) -> Self {
        Self {
            login_uri: login_uri.into(),
        }
    }
}

impl<S> Layer<S> for RequireLoginLayer {
    type Service = RequireLoginService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequireLoginService {
            inner,
            login_uri: self.login_uri.clone(),
        }
    }
}

#[derive(Clone)]
pub struct RequireLoginService<S> {
    pub inner: S,
    pub login_uri: String,
}

impl<S> Service<Request<Body>> for RequireLoginService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = ResponseFuture<S>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        //WARN: This Redirect doesn't work
        let redirect = Response::builder()
            .status(StatusCode::FOUND)
            .header(
                http::header::LOCATION,
                http::HeaderValue::from_str(&self.login_uri).expect("Failed to create HeaderValue"),
            )
            .body(Body::empty())
            .unwrap();

        let future = Box::pin(async {
            let Some(auth_session) = req.extensions().get::<AuthSession>() else {
                return Err(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .unwrap());
            };

            if auth_session.user.is_none() {
                return Err(redirect);
            }

            Ok(req)
        });

        ResponseFuture {
            state: State::Pending { future },
            inner: self.inner.clone(),
        }
    }
}
