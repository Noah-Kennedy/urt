use hashbrown::HashSet;
use std::collections::LinkedList;

pub(crate) struct Scheduler {
    queue: LinkedList<usize>,
    in_queue: HashSet<usize>,
}

impl Scheduler {
    pub(crate) fn new() -> Self {
        let queue = LinkedList::new();
        let in_queue = HashSet::new();

        Self { queue, in_queue }
    }

    pub(crate) fn spawn(&mut self, key: usize) -> usize {
        assert!(
            self.in_queue.insert(key),
            "Attempted to double-spawn task {key}"
        );

        self.queue.push_back(key);
        key
    }

    pub(crate) fn wake(&mut self, key: usize) {
        if !self.in_queue.contains(&key) {
            self.queue.push_back(key);
            self.in_queue.insert(key);
        }
    }

    pub(crate) fn fetch_next_task_for_tick(&mut self) -> Option<usize> {
        if let Some(key) = self.queue.pop_front() {
            assert!(
                self.in_queue.remove(&key),
                "Internal error: in_queue set did not contain removed key {key}",
            );

            Some(key)
        } else {
            None
        }
    }
}
