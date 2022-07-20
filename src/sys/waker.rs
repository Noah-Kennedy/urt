use crate::sys::CONTEXT;
use std::sync::Arc;
use std::task::{RawWaker, Wake};

pub struct SysWaker {
    key: usize,
}

impl SysWaker {
    pub(crate) fn new(key: usize) -> Self {
        Self { key }
    }
}

fn make_waker(key: usize) -> RawWaker {}

fn wake(key: usize) {
    CONTEXT.with(|x| {
        let borrow = x.borrow();
        let context = borrow.as_ref().unwrap();

        context.scheduler.borrow_mut().wake(self.key);
    })
}
