//! Tasks that come separated by time pauses.
//! A task can also be forced to be started at any time, but only during a pause.
//! If a task is forced to be started, the schedule of pauses modifies to accomodate this task.
//!
//! This code is more a demo of my `tokio-task-queue` than a serious module.

use std::borrow::Borrow;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use futures::future::{Fuse, FusedFuture, ready};
use futures::{ready, SinkExt, Stream, StreamExt, TryFutureExt};
use futures::FutureExt;
// use send_cell::SendCell;
use tokio::{select, spawn};
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time::{Sleep, sleep, timeout};
use tokio_interruptible_future::{InterruptError, interruptible, interruptible_sendable};
use async_trait::async_trait;
use crate::{TaskItem, TaskQueue};

pub struct TasksWithRegularPausesData {
    sudden_tx: Option<Arc<Mutex<Sender<()>>>>,
}

#[async_trait]
pub trait TasksWithRegularPauses: Send + Sync + 'static {
    fn data(&self) -> &TasksWithRegularPausesData;
    fn data_mut(&mut self) -> &mut TasksWithRegularPausesData;
    async fn next_task(&self) -> Option<TaskItem>;
    fn sleep_duration(&self) -> Duration;
    async fn _task(this: Arc<Mutex<Self>>) {
        loop {
            // It is time to run a task.
            let this1 = this.lock().await;
            let fut = this1.next_task().await;
            if let Some(fut) = fut {
                fut.await;
            } else {
                break;
            }

            // Signal that may interrupt the task.
            let (sudden_tx, mut sudden_rx) = mpsc::channel(1);
            this.lock().await.data_mut().sudden_tx = Some(Arc::new(Mutex::new(sudden_tx)));

            // Re-execute by a signal, or timeout (whichever comes first)
            let _ = timeout(Duration::from_secs(3600), sudden_rx.recv()).await;
        }
    }
    fn spawn(this: Arc<Mutex<Self>>) -> JoinHandle<()> {
        spawn(Self::_task(this))
    }
    async fn suddenly(this: Arc<Mutex<Self>>) -> Result<(), tokio::sync::mpsc::error::TrySendError<()>>{
        let mut this1 = this.lock().await;
        let sudden_tx = this1.data_mut().sudden_tx.take(); // atomic operation
        if let Some(sudden_tx) = sudden_tx {
            sudden_tx.lock().await.try_send(())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::iter::{repeat, repeat_with};
    use std::sync::Arc;
    use std::time::Duration;
    use futures::{Stream, stream};
    use tokio::sync::Mutex;
    use async_trait::async_trait;
    use tokio::sync::mpsc::channel;
    use crate::TaskItem;
    use crate::tasks_with_regular_pauses::{TasksWithRegularPauses, TasksWithRegularPausesData};

    struct OurTaskQueue {
        data: TasksWithRegularPausesData,
    }

    impl OurTaskQueue {
        pub fn new() -> Self {
            let (sudden_tx, sudden_rx) = oneshot::<()>();
            Self {
                data: TasksWithRegularPausesData {
                    sudden_tx: Arc::new(Mutex::new(sudden_tx)),
                    sudden_rx: Arc::new(Mutex::new(sudden_rx)),
                },
            }
        }
    }

    #[async_trait]
    impl<'a> TasksWithRegularPauses for OurTaskQueue where Self: 'static {
        fn data(&self) -> &TasksWithRegularPausesData {
            &self.data
        }
        fn data_mut(&mut self) -> &mut TasksWithRegularPausesData {
            &mut self.data
        }
        async fn next_task(&self) -> Option<TaskItem> {
            Some(Box::pin(async { () }))
        }
        fn sleep_duration(&self) -> Duration {
            Duration::from_millis(1)
        }
    }

    #[test]
    fn empty() {
        let tasks = stream::iter(repeat_with(|| Box::pin(async { () })));
        let tasks = Arc::new(Mutex::new(Box::pin(tasks)));
        let tasks_with_pauses = OurTaskQueue::new(tasks, Duration::from_millis(1));
    }
}