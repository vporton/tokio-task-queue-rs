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
use tokio::sync::{Mutex, Notify};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time::{Sleep, sleep, timeout};
use tokio_interruptible_future::{InterruptError, interruptible, interruptible_sendable};
use async_trait::async_trait;
use crate::{TaskItem, TaskQueue};

pub struct TasksWithRegularPausesData {
    sudden_tx: Arc<Mutex<Sender<()>>>,
    sudden_rx: Arc<Mutex<Receiver<()>>>,
}

#[async_trait]
pub trait TasksWithRegularPauses: Sync {
    fn data(&self) -> &TasksWithRegularPausesData;
    fn data_mut(&mut self) -> &mut TasksWithRegularPausesData;
    async fn next_task(&self) -> Option<TaskItem>;
    fn sleep_duration(&self) -> Duration;
    async fn _task(this: Arc<Mutex<Self>>) {
        let this2 = this.clone();
        loop {
            // It is time to run a task.
            let this1 = this.lock().await;
            let fut = this1.next_task().await;
            if let Some(fut) = fut {
                fut.await;
            } else {
                break;
            }

            // if the outdated signal is generated while download
            // was in progress, ignore the signal by draining the receiver
            loop {
                let this2 = this2.clone();
                let mut this1 = this.lock().await;
                let data = this1.data_mut();
                if data.sudden_rx.lock().await.try_recv().is_err() {
                    break;
                }
            }

            // re-download by a signal, or timeout (whichever comes first)
            let mut sudden_fut = { // block to shorten locks
                let this1 = this.lock().await;
                this1.data().sudden_rx.clone()
            };
            let _ = timeout(Duration::from_secs(3600), sudden_fut.lock().await.recv()).await;
        }
    }
    async fn suddenly(this: Arc<Mutex<Self>>) -> Result<(), tokio::sync::mpsc::error::SendError<()>>{
        let this1 = this.lock().await;
        this1.data().sudden_tx.lock().await.send(()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::iter::{repeat, repeat_with};
    use std::sync::Arc;
    use std::sync::mpsc::channel;
    use std::time::Duration;
    use futures::{Stream, stream};
    use tokio::sync::Mutex;
    use crate::TaskItem;
    use crate::tasks_with_regular_pauses::{TasksWithRegularPauses, TasksWithRegularPausesData};

    struct OurTaskQueue {
        data: TasksWithRegularPausesData,
    }

    impl OurTaskQueue {
        pub fn new() -> Self {
            let (sudden_tx, sudden_rx) = channel::<()>();
            Self {
                data: TasksWithRegularPausesData { sudden_tx, sudden_rx },
            }
        }
    }

    impl<'a> TasksWithRegularPauses for OurTaskQueue<'a> where Self: 'static {
        fn data(&'a self) -> &'a TasksWithRegularPausesData {
            &self.data
        }
        fn data_mut(&'a mut self) -> &'a mut TasksWithRegularPausesData {
            &mut self.data
        }
        async fn next_task(&self) -> Option<TaskItem> {
            Some(async { () })
        }
        fn sleep_duration(&self) -> Duration {
            Duration::from_millis(1)
        }
    }

    #[test]
    fn empty() {
        let tasks = stream::iter(repeat_with(|| Box::pin(async { () })));
        let tasks = Arc::new(Mutex::new(Box::pin(tasks)));
        let tasks_with_pauses = TasksWithRegularPauses::new(tasks, Duration::from_millis(1));
    }
}