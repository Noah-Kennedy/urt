use futures::pin_mut;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::oneshot;

pub(crate) struct Task {
    inner: Pin<Box<dyn Future<Output = ()> + Send + Sync>>,
}

pub struct JoinHandle<T> {
    inner: oneshot::Receiver<T>,
}

impl Task {
    pub(crate) fn new<F: 'static, T: 'static>(fut: F) -> (Task, JoinHandle<T>)
    where
        F: Future<Output = T> + Send + Sync,
        T: Send + Sync,
    {
        let (tx, rx) = oneshot::channel();

        let inner = Box::pin(async move {
            let out = fut.await;

            let _ = tx.send(out);
        });

        let task = Task { inner };

        let handle = JoinHandle { inner: rx };

        (task, handle)
    }

    pub(crate) fn poll_task(&mut self, cx: &mut Context<'_>) {
        let pinned = self.inner.as_mut();

        pin_mut!(pinned);

        let _ = pinned.poll(cx);
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
