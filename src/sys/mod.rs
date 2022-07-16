use io_uring::squeue;
use std::cell::RefCell;
use std::future::Future;
use std::io;
use std::rc::Rc;

mod scheduler;

mod task;

mod rt;

mod driver;

thread_local!(pub(crate) static CONTEXT: Option<ThreadContext> = None);

pub(crate) struct ThreadContext {
    spawner: Spawner,
    driver: Rc<RefCell<Driver>>,
}

pub fn spawn<T, F>(fut: F) -> JoinHandle<T>
where
    F: Future<Output = T> + 'static,
    T: Send + 'static + Sync,
{
    CONTEXT.with(|maybe| {
        let context = maybe.as_ref().unwrap();

        let (task, handle) = Task::new(fut);

        context.spawner.spawn(task);

        handle
    })
}

pub unsafe fn submit_op<T>(entry: squeue::Entry, data: T) -> io::Result<Op<T>>
where
    T: 'static,
{
    CONTEXT.with(|maybe| {
        let context = maybe.as_ref().unwrap();

        context.driver.borrow_mut().push(entry, data)
    })
}

use crate::task::JoinHandle;
pub(crate) use driver::*;
pub(crate) use rt::*;
pub(crate) use scheduler::*;
pub(crate) use task::*;
