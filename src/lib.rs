use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use futures::stream::Stream;
use futures::StreamExt;
use tokio::spawn;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;
use tokio_interruptible_future::{InterruptError, interruptible};

type TaskItem = Box<dyn Future<Output = ()> + Send + Unpin>;

pub struct TaskQueue<TaskStream: Stream<Item = TaskItem> + Send + 'static>
{
    task_stream: Pin<Box<TaskStream>>,
}

impl<TaskStream: Stream<Item = TaskItem> + Send + 'static> TaskQueue<TaskStream>
{
    pub fn new(task_stream: TaskStream) -> Self {
        Self {
            task_stream: Box::pin(task_stream),
        }
    }
    async fn _task(this: Arc<Mutex<Self>>) { // FIXME: Check everything
        loop {
            let this2 = this.clone();
            let stream = &mut *this2.lock().await;
            if let Some(task) = stream.task_stream.next().await {
                task.await;
            } else {
                break;
            }
        }
    }
    pub fn spawn(this: Arc<Mutex<Self>>, notify_interrupt: Arc<Notify>) -> JoinHandle<Result<(), InterruptError>> {
        spawn( interruptible(notify_interrupt, async move {
            Self::_task(this).await;
            Ok(())
        }))
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
