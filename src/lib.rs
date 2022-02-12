mod tasks_with_regular_pauses;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use futures::stream::Stream;
use futures::StreamExt;
use tokio::spawn;
use tokio::sync::{Mutex, Notify};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_interruptible_future::{InterruptError, interruptible};


pub type TaskItem = Pin<Box<dyn Future<Output = ()> + Send + Unpin>>;

/// Execute futures from a stream of futures in order in a Tokio task. Not tested code.
pub struct TaskQueue
{
    tx: Sender<TaskItem>,
    rx: Arc<Mutex<Receiver<TaskItem>>>,
}

impl TaskQueue {
    pub fn new() -> Self {
        let (tx, rx) = channel(1);
        Self {
            tx,
            rx: Arc::new(Mutex::new(rx)),
        }
    }
    async fn _task(this: Arc<Mutex<Self>>) {
        // let mut rx = ReceiverStream::new(rx);
        loop {
            let this2 = this.clone();
            let fut = { // block to shorten locks lifetime
                let obj = this2.lock().await;
                let rx = obj.rx.clone();
                let mut rx = rx.lock().await;
                rx.recv().await
            };
            if let Some(fut) = fut {
                fut.await;
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
    pub fn push_task(&self, fut: TaskItem) {
        self.tx.send(fut);
    }
}
