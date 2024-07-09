use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use axum::body::Body;
use futures_util::future::BoxFuture;
use futures_util::Future;
use http::{Request, Response};
use pin_project_lite::pin_project;
use tower::{Layer, Service};

type MiddlewareFn = Arc<
    dyn Fn(Request<Body>) -> BoxFuture<'static, Result<Request<Body>, Response<Body>>>
        + Send
        + Sync
        + 'static,
>;

pub struct MiddlewareLayer {
    func: MiddlewareFn,
}

impl MiddlewareLayer {
    pub fn new(func: MiddlewareFn) -> Self {
        Self { func }
    }
}

impl<S> Layer<S> for MiddlewareLayer {
    type Service = MiddlewareService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MiddlewareService {
            inner: Arc::new(Mutex::new(inner)),
            func: Arc::clone(&self.func),
        }
    }
}

/// Constructs a middleware stack from one or more functions of type
/// `Fn(Request<Body>) -> impl Future<Output = Result<Request<Body>, Response<Body>>>`.
///
/// The stack is executed such that the rightmost functions are wrapped by the leftmost ones.
/// This means `compose_from_fn!(outer_fn, inner_fn)` will result in
/// `outer_fn` executing before `inner_fn`.
#[macro_export]
macro_rules! compose_from_fn {
    ($($func:expr),+) => {{
        use std::sync::Arc;
        use futures_util::future::{BoxFuture, FutureExt};
        use http::{Request, Response };
        use axum::body::Body;

        // Returns a single MiddlewareFn from a vector of MiddlewareFn
        // composing the middleware stack
        let composite_func = {
            Arc::new(move |req: Request<Body>| {
                let funcs = vec![$(move |req| ($func)(req).boxed()),*];
                let mut future = Box::pin(async move { Ok(req) })
                    as BoxFuture<'static, Result<Request<Body>, Response<Body>>>;

                for func in funcs.into_iter() {
                    let prev_future = future;
                    future = Box::pin(async move {
                        let req = prev_future.await?;
                        func(req).await
                    });
                }

                future
            })
        };

        $crate::middlewares::MiddlewareLayer::new(composite_func)
    }};
}

pub struct MiddlewareService<S> {
    inner: Arc<Mutex<S>>,
    func: MiddlewareFn,
}

impl<S> Service<Request<Body>> for MiddlewareService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Send + 'static,
    S::Future: Send,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = MiddlewareFuture<S>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let mut inner = self.inner.lock().unwrap();
        inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let inner = Arc::clone(&self.inner);
        let func = Arc::clone(&self.func);

        let future = Box::pin(async move { func(req).await });

        MiddlewareFuture {
            state: State::Pending { future },
            inner,
        }
    }
}

pin_project! {
    pub struct MiddlewareFuture<S> where S: Service<Request<Body>> {
        #[pin]
        pub state: State<BoxFuture<'static, Result<Request<Body>, Response<Body>>>, S::Future>,
        pub inner: Arc<Mutex<S>>,
    }
}

pin_project! {
    #[project = FutState]
    enum State<SvcFut, NxtFut> {
        Pending {
            #[pin]
            future: SvcFut,
        },
        Done {
            #[pin]
            next: NxtFut,
        }
    }
}

impl<S> Future for MiddlewareFuture<S>
where
    S: Service<Request<Body>, Response = Response<Body>>,
{
    type Output = Result<Response<Body>, S::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            match this.state.as_mut().project() {
                FutState::Pending { future } => {
                    let result = std::task::ready!(future.poll(cx));
                    match result {
                        Ok(req) => {
                            let mut inner = this.inner.lock().unwrap();
                            let (parts, body) = req.into_parts();
                            leptos::provide_context(parts.clone());
                            let next = inner.call(Request::from_parts(parts, body));
                            this.state.set(State::Done { next })
                        }
                        Err(res) => {
                            return Poll::Ready(Ok(res));
                        }
                    };
                }
                FutState::Done { next } => {
                    return next.poll(cx);
                }
            }
        }
    }
}
