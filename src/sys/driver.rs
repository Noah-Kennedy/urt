use io_uring::IoUring;
use slab::Slab;
use std::any::Any;
use std::task::Waker;

pub(crate) enum Lifetime {
    Submitted,
    InFlight,
    Cancelled(Box<dyn Any>),
}

pub(crate) struct Driver {
    slab: Slab<(Lifetime, Waker)>,
    uring: IoUring,
}
