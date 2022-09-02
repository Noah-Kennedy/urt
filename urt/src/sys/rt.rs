use crate::sys::{Driver, Scheduler, Task};

use crate::sys::waker::make_waker;
use slab::Slab;
use std::cell::RefCell;
use std::rc::Rc;

use std::collections::VecDeque;
use std::task::Context;
use std::{io, mem};

pub(crate) struct Worker {
    tasks: Slab<Task>,
    scheduler: Rc<RefCell<Scheduler>>,
    spawner: Spawner,
    driver: Rc<RefCell<Driver>>,
}

enum Tick {
    Poll,
    QueueEmpty,
    TasksEmpty,
}

#[derive(Clone)]
pub(crate) struct Spawner {
    sender: Rc<RefCell<VecDeque<Task>>>,
}

impl Worker {
    pub(crate) fn new(driver: Rc<RefCell<Driver>>) -> Self {
        let tasks = Slab::with_capacity(4096);
        let scheduler = Rc::new(RefCell::new(Scheduler::new()));

        let spawner = Spawner {
            sender: Rc::new(RefCell::new(VecDeque::with_capacity(4096))),
        };

        Self {
            tasks,
            scheduler,
            spawner,
            driver,
        }
    }

    pub(crate) fn driver(&self) -> Rc<RefCell<Driver>> {
        self.driver.clone()
    }

    pub(crate) fn spawner(&self) -> Spawner {
        self.spawner.clone()
    }

    pub(crate) fn scheduler(&self) -> Rc<RefCell<Scheduler>> {
        self.scheduler.clone()
    }

    pub(crate) fn run(&mut self) -> io::Result<()> {
        let mut polled_counter = 0u64;

        loop {
            if polled_counter == 128 {
                polled_counter = 0;
                self.poll()?;
            }

            match self.tick() {
                Tick::Poll => {
                    polled_counter += 1;
                }
                Tick::QueueEmpty => self.park()?,
                Tick::TasksEmpty => {
                    return Ok(());
                }
            }
        }
    }

    #[inline]
    fn poll(&mut self) -> io::Result<()> {
        self.driver.borrow_mut().poll()
    }

    fn park(&mut self) -> io::Result<()> {
        self.driver.borrow_mut().park()
    }

    fn tick(&mut self) -> Tick {
        let mut guard = self.scheduler.borrow_mut();

        // intake new tasks if present
        while let Some(t) = self.spawner.sender.borrow_mut().pop_front() {
            let key = self.tasks.insert(t);
            guard.spawn(key);
        }

        // try and poll a task if available
        if let Some(key) = guard.fetch_next_task_for_tick() {
            let waker = make_waker(key);

            if let Some(task) = self.tasks.get_mut(key) {
                mem::drop(guard);

                let mut cx = Context::from_waker(&waker);

                if task.poll_task(&mut cx) {
                    self.tasks.remove(key);
                }

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
}

impl Spawner {
    pub(crate) fn spawn(&self, t: Task) {
        let _ = self.sender.borrow_mut().push_back(t);
    }
}
