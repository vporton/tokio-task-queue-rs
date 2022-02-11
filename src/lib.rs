mod tasks_with_regular_pauses;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use futures::stream::Stream;
use futures::StreamExt;
use tokio::spawn;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;
use tokio_interruptible_future::{InterruptError, interruptible};

pub type TaskItem = Pin<Box<dyn Future<Output = ()> + Send + Unpin>>;

/// Execute futures from a stream of futures in order in a Tokio task. Not tested code.
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
    async fn _task(this: Arc<Mutex<Self>>) {
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
}
