use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::{select, spawn};
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;

pub struct TaskQueue {
    // join_handle: JoinHandle<()>,
    queued_tasks: VecDeque<Pin<Box<dyn Future<Output = ()> + Send>>>,
    notify: Arc<Mutex<Notify>>, // Used for both notifications after a task queued and for closing the thread.
}

impl TaskQueue {
    pub fn new() -> Self {
        Self {
            queued_tasks: VecDeque::new(),
            notify: Arc::new(Mutex::new(Notify::new())),
        }
    }
    async fn _task(this: Arc<Mutex<Self>>) { // FIXME: Check everything
        let notify = this.lock().await.notify.clone();
        let front = this.lock().await.queued_tasks.pop_front();
        if let Some(front) = front {
            // Not all `push_task` notifications handled, we will return on a subsequent loop iteration.
            // So, no notification is lost.
            let notify_guard = notify.lock().await;
            select! {
                _ = notify_guard.notified() => {
                    // All notifications by `push_task` are already handled (otherwise, it wouldn't be empty),
                    // so, it is a notification to interrupt.
                    return;
                }
                _ = front => { }
            };
        } else {
            notify.lock().await.notified().await
        }
    }
    pub async fn push_task(this: Arc<Mutex<Self>>, task: Pin<Box<dyn Future<Output=()> + Send>>) {
        this.lock().await.queued_tasks.push_back(task);
        this.lock().await.notify.lock().await.notify_one(); // notify_waiters() doesn't work here, because it would need already to wait.
    }
    pub fn spawn(this: Arc<Mutex<Self>>) -> JoinHandle<()> {
        spawn(Self::_task(this))
    }
    /// If notified when task queue is empty, stops the scheduler.
    pub async fn notify(&self) {
        self.notify.lock().await.notify_one();
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
            queue.lock().await.notify().await; // Stop the scheduler.
            join_handle.await.unwrap();
        });
    }
}
