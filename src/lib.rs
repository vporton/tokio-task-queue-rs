use log::debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::{select, spawn};
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;

pub struct TaskQueue {
    // join_handle: JoinHandle<()>,
    queued_tasks: Vec<Pin<Box<dyn Future<Output = ()> + Send>>>,
    notify: Notify, // Used for both notifications after a task queued and for closing the thread.
}

impl TaskQueue {
    pub fn new() -> Self {
        Self {
            queued_tasks: Vec::new(),
            notify: Notify::new(),
        }
    }
    async fn _task(this: Arc<Mutex<Self>>) { // FIXME: Check everything
        debug!("TASK");
        let guard = this.lock().await;
        debug!("guard.queued_tasks.len() = {}", guard.queued_tasks.len());
        if !this.lock().await.queued_tasks.is_empty() {
            // Not all `push_task` notifications handled, we will return on a subsequent loop iteration.
            // So, no notification is lost.
            let front = this.lock().await.queued_tasks.remove(0);
            select! {
                _ = guard.notify.notified() => {
                    // All notifications by `push_task` are already handled (otherwise, it wouldn't be empty),
                    // so, it is a notification to interrupt.
                    return;
                }
                _ = front => { }
            };
        } else {
            guard.notify.notified().await
        }

        debug!("UUU");
    }
    pub async fn push_task(this: Arc<Mutex<Self>>, task: Pin<Box<dyn Future<Output=()> + Send>>) {
        this.lock().await.queued_tasks.push(task);
        this.lock().await.notify.notify_one(); // notify_waiters() doesn't work here, because it would need already to wait.
    }
    pub fn spawn(this: Arc<Mutex<Self>>) -> JoinHandle<()> {
        debug!("RRR");
        spawn(Self::_task(this))
    }
    /// If notified when task queue is empty, stops the scheduler.
    pub fn notify(&self) {
        self.notify.notify_waiters();
    }
    // pub fn notifier(&self) -> &Notify { // I don't expose it to public API because of mess in notify_waiters()
    //     &self.notify
    // }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tokio::runtime::Runtime;
    use tokio::sync::Mutex;
    use crate::TaskQueue;

    #[test]
    fn test() {
        let rt  = Runtime::new().unwrap();
        rt.block_on(async {
            let queue = Arc::new(Mutex::new(TaskQueue::new()));
            TaskQueue::push_task(queue.clone(), Box::pin(async { let _ = 1 + 1; })).await;
            let join_handle = TaskQueue::spawn(queue.clone());
            TaskQueue::push_task(queue.clone(), Box::pin(async { let _ = 2 + 2; })).await;
            queue.lock().await.notify(); // Stop the scheduler.
            join_handle.await.unwrap();
        });
    }
}
