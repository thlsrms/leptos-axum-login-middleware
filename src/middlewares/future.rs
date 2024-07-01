use std::pin::Pin;
use std::task::{Context, Poll};

use axum::body::Body;
use futures_util::future::BoxFuture;
use futures_util::Future;
use http::{Request, Response};
use pin_project_lite::pin_project;
use tower::Service;

pin_project! {
    pub struct ResponseFuture<S> where S: Service<Request<Body>> {
        #[pin]
        pub state: State<BoxFuture<'static, Result<Request<Body>, Response<Body>>>, S::Future>,
        pub inner: S,
    }
}

pin_project! {
    #[project = FutState]
    pub enum State<SvcFut, NxtFut> {
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

impl<S> Future for ResponseFuture<S>
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
                            let next = this.inner.call(req);
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
