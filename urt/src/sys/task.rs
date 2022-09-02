use crate::task::JoinHandle;
use futures::pin_mut;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use tokio::sync::oneshot;

pub(crate) struct Task {
    inner: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    pub(crate) fn new<F: 'static, T: 'static>(fut: F) -> (Task, JoinHandle<T>)
    where
        F: Future<Output = T>,
        T: Send + Sync,
    {
        let (tx, rx) = oneshot::channel();

        let inner = Box::pin(async move {
            let out = fut.await;

            let _ = tx.send(out);
        });

        let task = Task { inner };

        let handle = JoinHandle::new(rx);

        (task, handle)
    }

    pub(crate) fn poll_task(&mut self, cx: &mut Context<'_>) -> bool {
        let pinned = self.inner.as_mut();

        pin_mut!(pinned);

        pinned.poll(cx).is_ready()
    }
}
