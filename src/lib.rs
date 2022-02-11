use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use futures::future::ready;
use futures::stream::Stream;
use futures::StreamExt;
use tokio::{select, spawn};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

struct TaskItem {
    task: Pin<Box<dyn Future<Output = ()> + Send + Unpin>>,
    interrupt_previous: bool,
}

// FIXME: To overcome the race condition, use a tx/rx pair instead of deque.
pub struct TaskQueue<TaskStream: Stream<Item = TaskItem> + Send + 'static>
{
    // join_handle: JoinHandle<()>,
    task_stream: Pin<Box<TaskStream>>,
    current_task: Option<Pin<Box<dyn Future<Output = ()> + Send + Unpin>>>,
}

impl<TaskStream: Stream<Item = TaskItem> + Send + 'static> TaskQueue<TaskStream>
{
    pub fn new(task_stream: TaskStream) -> Self {
        Self {
            current_task: None,
            task_stream: Box::pin(task_stream),
        }
    }
    async fn _task(this: Arc<Mutex<Self>>) { // FIXME: Check everything
        let this2 = this.clone();
        loop {
            let this2 = this2.clone();
            let finish_current = async move {
                let current_task = { // block to limit guard
                    let mut guard = this2.lock().await;
                    guard.current_task.take()
                };
                if let Some(current) = current_task {
                    current.await;
                }
            };
            if let Some(current) = &this.lock().await.current_task {
                select! {
                    _ = *current => { }
                    front = this.lock().await.task_stream.next() => {
                        if let Some(front) = front {
                            if !front.interrupt_previous {
                                finish_current.await;
                            }
                            this.lock().await.current_task = Some(front.task);
                            this.lock().await.current_task.unwrap().await;
                        } else {
                            finish_current.await;
                            return;
                        }
                    }
                };
            } else {
                this.lock().await.task_stream.next().await;
                finish_current.await;
            }
        }
    }
    // pub async fn push_task(this: Arc<Mutex<Self>>, task: Pin<Box<dyn Future<Output=()> + Send>>) {
    //     this.lock().await.queued_tasks.push_back(task);
    //     this.lock().await.notify_interrupt.lock().await.notify_one(); // notify_waiters() doesn't work here, because it would need already to wait.
    // }
    pub fn spawn(this: Arc<Mutex<Self>>) -> JoinHandle<()> {
        spawn(Self::_task(this)) // TODO: Make it interruptible.
    }
    // If notified when task queue is empty, stops the scheduler.
    // pub async fn notify(&self) {
    //     self.notify_interrupt_queue.lock().await.notify_one();
    // }
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
