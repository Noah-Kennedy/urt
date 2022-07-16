use crate::sys::{Driver, Task, ThreadContext, Worker, CONTEXT};
pub use crate::task::JoinHandle;
use std::cell::RefCell;
use std::future::Future;
use std::io;
use std::rc::Rc;

pub struct Runtime {
    worker: Worker,
}

impl Runtime {
    pub fn new(entries: u32) -> io::Result<Self> {
        let driver = Rc::new(RefCell::new(Driver::new(entries)?));
        let worker = Worker::new(driver);
        Ok(Self { worker })
    }

    pub fn spawn<T, F>(&self, fut: F) -> JoinHandle<T>
    where
        F: Future<Output = T> + 'static,
        T: Send + 'static + Sync,
    {
        let (task, handle) = Task::new(fut);

        self.worker.spawner().spawn(task);

        handle
    }

    pub fn run<T, F>(&mut self) -> io::Result<()> {
        // exit context when this finishes
        struct ContextGuard {}

        impl Drop for ContextGuard {
            fn drop(&mut self) {
                CONTEXT.with(|x| {
                    let mut guard = x.borrow_mut();

                    *guard = None;
                });
            }
        }

        // enter context
        CONTEXT.with(|x| {
            let mut guard = x.borrow_mut();

            *guard = Some(ThreadContext {
                spawner: self.worker.spawner(),
                driver: self.worker.driver(),
            });
        });

        self.worker.run()
    }
}
