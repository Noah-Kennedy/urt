use crate::task::{JoinHandle, Task};
use parking_lot::Mutex;
use slab::Slab;
use std::cell::Cell;
use std::collections::LinkedList;
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Wake, Waker};

thread_local!(pub(crate) static CURRENT_TASK: Cell<usize> = Cell::new(usize::MAX));

pub(crate) struct Scheduler {
    ready_tasks: Arc<Mutex<LinkedList<usize>>>,
    reactor: Arc<Mutex<Slab<Task>>>,
}

pub(crate) struct TaskWaker {
    ready_tasks: Arc<Mutex<LinkedList<usize>>>,
    key: usize,
}

impl Scheduler {
    pub(crate) fn new() -> Self {
        Self {
            ready_tasks: Arc::new(Mutex::new(LinkedList::new())),
            reactor: Arc::new(Mutex::new(Slab::new())),
        }
    }

    pub(crate) fn spawn<F: 'static, T: 'static>(&mut self, fut: F) -> JoinHandle<T>
    where
        F: Future<Output = T> + Send + Sync,
        T: Send + Sync,
    {
        let (task, handle) = Task::new(fut);

        let key = self.reactor.lock().insert(task);

        self.ready_tasks.lock().push_back(key);

        handle
    }

    pub(crate) fn tick(&self) -> bool {
        let mut tasks_list_guard = self.ready_tasks.lock();
        let mut reactor_guard = self.reactor.lock();

        if let Some(key) = tasks_list_guard.pop_front() {
            let waker = self.task_waker(key);
            let task = reactor_guard.get_mut(key).unwrap();

            CURRENT_TASK.with(|x| x.set(key));
            task.poll_task(&mut Context::from_waker(&waker));

            true
        } else {
            false
        }
    }

    fn task_waker(&self, key: usize) -> Waker {
        Arc::new(TaskWaker {
            ready_tasks: self.ready_tasks.clone(),
            key,
        })
        .into()
    }
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        let mut guard = self.ready_tasks.lock();

        guard.push_back(self.key);
    }
}
