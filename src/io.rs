use crate::submit_op;
use crate::sys::Op;
use futures::{pin_mut, ready};
use io_uring::squeue::Flags;
use io_uring::{cqueue, squeue};
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct Unsubmitted<D, O, F>
where
    F: FnOnce(cqueue::Entry, D) -> io::Result<O>,
{
    entry: squeue::Entry,
    data: D,
    post_op: F,
}

pub struct Submitted<D, O, F>
where
    D: 'static,
    F: FnOnce(cqueue::Entry, D) -> io::Result<O>,
{
    op: Op<D>,
    post_op: Option<F>,
}

impl<D, O, F> Future for Submitted<D, O, F>
where
    F: FnOnce(cqueue::Entry, D) -> io::Result<O> + Unpin,
    D: Unpin + 'static,
{
    type Output = io::Result<O>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let op = &mut this.op;
        pin_mut!(op);

        let (entry, data) = ready!(op.poll(cx));

        let post_op = this.post_op.take().expect("Polled after completed");

        Poll::Ready(post_op(entry, data))
    }
}

impl<D, O, F> Unsubmitted<D, O, F>
where
    F: FnOnce(cqueue::Entry, D) -> io::Result<O>,
    D: Unpin + 'static,
{
    pub unsafe fn from_raw(entry: squeue::Entry, data: D, post_op: F) -> Self {
        Self {
            entry,
            data,
            post_op,
        }
    }

    pub unsafe fn apply_flags(&mut self, flags: Flags) {
        self.entry = self.entry.clone().flags(flags);
    }

    pub fn submit(self) -> io::Result<Submitted<D, O, F>> {
        let op = unsafe { submit_op(self.entry, self.data)? };

        Ok(Submitted {
            op,
            post_op: Some(self.post_op),
        })
    }
}
