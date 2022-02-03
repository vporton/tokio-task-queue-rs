use std::future::Future;
use std::pin::Pin;
use tokio::spawn;
use tokio::sync::Notify;
use tokio::task::{JoinHandle, spawn_local};

pub struct TaskQueue {
    // join_handle: JoinHandle<()>,
    queued_tasks: Vec<Pin<Box<dyn Future<Output = ()> + Send  + 'static>>>,
    notify: Notify,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self {
            queued_tasks: Vec::new(),
            notify: Notify::new(),
        }
    }
    async fn _task(&mut self) {
        self.notify.notified().await;
        while !self.queued_tasks.is_empty() {
            let front = self.queued_tasks.remove(0);
            spawn_local(front);
        }
    }
    pub fn push_task(&mut self, task: Pin<Box<dyn Future<Output=()> + Send  + 'static>>) {
        self.queued_tasks.push(task);
        self.notify.notify_waiters();
    }
    pub fn spawn(self: &'static mut Self) -> JoinHandle<()> {
        spawn(async { Self::_task(self).await; })
    }
    /// Can be used to stop waiting for no more tasks:
    pub fn notify(&self) {
        self.notify.notify_waiters();
    }
    pub fn notifier(&self) -> &Notify {
        &self.notify
    }
}

#[cfg(test)]
mod tests {
    use crate::TaskQueue;

    #[test]
    fn test() {
        let mut queue = TaskQueue::new();
        queue.push_task(Box::pin(async { 1 + 1; }));
        let join_handle = queue.spawn();
        queue.push_task(Box::pin(async { 2 + 2; }));
        // block_on(join_handle);
    }
}
