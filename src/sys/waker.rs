use crate::sys::CONTEXT;
use std::task::{RawWaker, RawWakerVTable, Waker};

const VTABLE: RawWakerVTable = RawWakerVTable::new(make_raw_waker, wake, wake, drop_waker);

pub(crate) fn make_waker(key: usize) -> Waker {
    let key = key as *const ();
    unsafe { Waker::from_raw(make_raw_waker(key)) }
}

const fn make_raw_waker(key: *const ()) -> RawWaker {
    RawWaker::new(key, &VTABLE)
}

fn wake(key: *const ()) {
    let key = key as usize;

    CONTEXT.with(|x| {
        let borrow = x.borrow();
        let context = borrow.as_ref().unwrap();

        context.scheduler.borrow_mut().wake(key);
    })
}

fn drop_waker(_: *const ()) {}
