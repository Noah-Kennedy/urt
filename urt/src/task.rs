use futures::pin_mut;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::oneshot;

pub struct JoinHandle<T> {
    inner: oneshot::Receiver<T>,
}

impl<T> JoinHandle<T> {
    pub(crate) fn new(inner: oneshot::Receiver<T>) -> Self {
        Self { inner }
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pinned = self.get_mut();

        let inner = &mut pinned.inner;

        pin_mut!(inner);

        inner.poll(cx).map(|x| x.unwrap())
    }
}
