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
use crate::{TaskQueue};

pub struct TasksWithRegularPausesData {
    sudden_tx: Sender<()>,
    sudden_rx: Receiver<()>,
}

// FIXME: Should instead use from `lib.rs`.
pub type TaskItem = Pin<Box<dyn Future<Output = ()> + Send + Sync>>;


#[async_trait]
trait TasksWithRegularPauses<'a> where Self: Sync + 'static{
    fn data(&'a self) -> &'a TasksWithRegularPausesData;
    fn data_mut(&'a mut self) -> &'a mut TasksWithRegularPausesData;
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
                let mut this1 = this2.lock().await;
                if this1.data_mut().sudden_rx.try_recv().is_err() {
                    break;
                }
            }
            // while {
            //     let mut this1 = this2.lock().await;
            //     this1.data_mut().sudden_rx.try_recv().is_ok()
            // } { }

            // re-download by a signal, or timeout (whichever comes first)
            timeout(Duration::from_secs(3600), this.lock().await.data_mut().sudden_rx.recv()).await;
        }
    }
    async fn suddenly(this: Arc<Mutex<Self>>) {
        let this1 = this.lock().await;
        this1.data().sudden_tx.send(());
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