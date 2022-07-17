use io_uring::{cqueue, squeue, IoUring};
use slab::Slab;
use std::any::Any;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::{io, mem};

pub(crate) enum Lifetime {
    Submitted,
    Waiting(Waker),
    Cancelled(Box<dyn Any>),
    Completed(io_uring::cqueue::Entry),
}

pub(crate) struct Driver {
    slab: Rc<RefCell<Slab<Lifetime>>>,
    uring: IoUring,
}

impl Driver {
    pub(crate) fn new(entries: u32) -> io::Result<Self> {
        let slab = Rc::new(RefCell::new(Slab::new()));

        let uring = io_uring::Builder::default().dontfork().build(entries)?;

        Ok(Self { slab, uring })
    }

    #[inline]
    pub(crate) unsafe fn push<T>(&mut self, entry: squeue::Entry, data: T) -> io::Result<Op<T>>
    where
        T: 'static,
    {
        let mut guard = self.slab.borrow_mut();

        let vacant = guard.vacant_entry();

        let key = vacant.key();

        let entry = entry.user_data(key as _);

        while self.uring.submission().push(&entry).is_err() {
            self.uring.submit()?;
        }

        vacant.insert(Lifetime::Submitted);

        Ok(Op {
            slab: self.slab.clone(),
            data: Some(data),
            key,
        })
    }

    #[inline]
    pub(crate) fn get_remaining(&self) -> usize {
        unsafe { self.uring.submission_shared().capacity() - self.uring.submission_shared().len() }
    }

    pub(crate) fn poll(&mut self) -> io::Result<()> {
        self.uring.submit()?;

        self.complete();

        Ok(())
    }

    pub(crate) fn park(&mut self) -> io::Result<()> {
        if !self.complete() {
            self.uring.submit_and_wait(1)?;

            self.complete();
        } else {
            self.uring.submit()?;
        }

        Ok(())
    }

    pub(crate) fn complete(&mut self) -> bool {
        let mut completions = self.uring.completion();
        let mut slab = self.slab.borrow_mut();

        completions.sync();

        let res = !completions.is_empty();

        for c in completions {
            let key = c.user_data() as usize;

            let old_lifetime = mem::replace(slab.get_mut(key).unwrap(), Lifetime::Completed(c));

            match old_lifetime {
                Lifetime::Submitted => {}
                Lifetime::Waiting(waker) => {
                    waker.wake();
                }
                Lifetime::Cancelled(_) => {
                    let _ = slab.remove(key);
                }
                Lifetime::Completed(_) => {
                    unimplemented!("Handle multi-shot ops");
                }
            }
        }

        res
    }
}

pub struct Op<T>
where
    T: 'static,
{
    slab: Rc<RefCell<Slab<Lifetime>>>,
    data: Option<T>,
    key: usize,
}

impl<T> Future for Op<T>
where
    T: Unpin + 'static,
{
    type Output = (cqueue::Entry, T);

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;

        let mut slab = this.slab.borrow_mut();

        let lifetime = slab.get_mut(this.key).unwrap();

        let results = match lifetime {
            Lifetime::Submitted => {
                let new_lifetime = Lifetime::Waiting(cx.waker().clone());

                let _ = mem::replace(lifetime, new_lifetime);

                None
            }
            Lifetime::Waiting(waker) => {
                if !cx.waker().will_wake(waker) {
                    let _ = mem::replace(waker, cx.waker().clone());
                }

                None
            }
            Lifetime::Cancelled(_) => {
                panic!("How are we polling a canceled op?");
            }
            Lifetime::Completed(entry) => Some(entry.clone()),
        };

        if let Some(entry) = results {
            let _ = slab.remove(this.key);

            Poll::Ready((entry, this.data.take().unwrap()))
        } else {
            Poll::Pending
        }
    }
}

impl<T> Drop for Op<T>
where
    T: 'static,
{
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            let mut slab = self.slab.borrow_mut();

            let lifetime = mem::replace(
                slab.get_mut(self.key).unwrap(),
                Lifetime::Cancelled(Box::new(data)),
            );

            if matches!(lifetime, Lifetime::Completed(_)) {
                let _ = slab.remove(self.key);
            }
        }
    }
}
