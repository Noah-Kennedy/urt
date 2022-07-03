use crate::sys::{Scheduler, Task};
use crossbeam_channel::{Receiver, Sender};
use slab::Slab;
use std::mem;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Wake, Waker};

pub(crate) struct Worker {
    tasks: Slab<Task>,
    scheduler: Arc<Mutex<Scheduler>>,
    new_tasks: Receiver<Task>,
    spawner: Spawner,
}

struct SysWaker {
    key: usize,
    scheduler: Weak<Mutex<Scheduler>>,
}

enum Tick {
    Poll,
    QueueEmpty,
    TasksEmpty,
}

#[derive(Clone)]
pub(crate) struct Spawner {
    sender: Sender<Task>,
}

impl Worker {
    pub(crate) fn new() -> Self {
        let tasks = Slab::new();
        let scheduler = Arc::new(Mutex::new(Scheduler::new()));

        let (sender, new_tasks) = crossbeam_channel::unbounded();

        let spawner = Spawner { sender };

        Self {
            tasks,
            scheduler,
            new_tasks,
            spawner,
        }
    }

    pub(crate) fn spawner(&self) -> Spawner {
        self.spawner.clone()
    }

    pub(crate) fn run(&mut self) {
        let mut polled_counter = 0u64;

        loop {
            if polled_counter == 127 {
                polled_counter = 0;
                self.drive();
            }

            match self.tick() {
                Tick::Poll => {
                    polled_counter += 1;
                }
                Tick::QueueEmpty => self.park(),
                Tick::TasksEmpty => {
                    return;
                }
            }
        }
    }

    fn drive(&mut self) {
        todo!()
    }

    fn park(&mut self) {
        todo!()
    }

    fn tick(&mut self) -> Tick {
        let mut guard = self.scheduler.lock().unwrap();

        // intake new tasks if present
        if let Ok(t) = self.new_tasks.try_recv() {
            let key = self.tasks.insert(t);
            guard.spawn(key);
        }

        // try and poll a task if available
        if let Some(key) = guard.fetch_next_task_for_tick() {
            let waker = self.waker(key);

            if let Some(task) = self.tasks.get_mut(key) {
                mem::drop(guard);

                let mut cx = Context::from_waker(&waker);

                task.poll_task(&mut cx);

                return Tick::Poll;
            }
        }

        // if we couldn't poll, figure out why
        if self.tasks.is_empty() {
            Tick::TasksEmpty
        } else {
            Tick::QueueEmpty
        }
    }

    fn waker(&self, key: usize) -> Waker {
        let scheduler = Arc::downgrade(&self.scheduler);
        let uwaker = SysWaker { key, scheduler };

        Waker::from(Arc::new(uwaker))
    }
}

impl Spawner {
    pub(crate) fn spawn(&self, t: Task) {
        let _ = self.sender.send(t);
    }
}

impl Wake for SysWaker {
    fn wake(self: Arc<Self>) {
        if let Some(unlocked) = self.scheduler.upgrade() {
            let mut guard = unlocked.lock().unwrap();

            guard.wake(self.key);
        }
    }
}