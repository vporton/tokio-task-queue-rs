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
use tokio::sync::mpsc::channel;
use tokio::task::JoinHandle;
use tokio::time::{Sleep, sleep};
use tokio_interruptible_future::{InterruptError, interruptible, interruptible_sendable};
use async_trait::async_trait;
use crate::{TaskItem, TaskQueue};

pub struct TasksWithRegularPausesData {
    task_queue: Arc<Mutex<TaskQueue>>,
    pause_interrupt_tx: Option<async_channel::Sender<()>>, // `None` when not in pause
}

#[async_trait]
trait TasksWithRegularPauses<'a> where Self: 'static {
    fn data(&self) -> &'a TasksWithRegularPausesData;
    fn data_mut(&mut self) -> &'a mut TasksWithRegularPausesData;
    async fn next_task(&self) -> Option<TaskItem>;
    fn sleep_duration(&self) -> Duration;
    async fn _task(this: Arc<Mutex<Self>>) {
        let this2 = this.clone();
        loop {
            { // block
                let task = { // block to shorten lifetimes
                    let this1 = this.lock().await; // FIXME: lock for too long?
                    let task_fut = this1.next_task();
                    task_fut.await
                };
                if let Some(task) = task { // FIXME: lock duration?
                    this.lock().await.data_mut().task_queue.lock().await.push_task(Box::pin(task)).await;
                } else {
                    break;
                }
            }

            let sleep_duration = this.lock().await.sleep_duration();
            let (pause_interrupt_tx, pause_interrupt_rx) = async_channel::bounded(1);
            let notify_end_sleep_pair = Arc::new(Mutex::new(async_channel::bounded(1)));
            let this2 = this2.clone();
            let notify_end_sleep_pair2 = notify_end_sleep_pair.clone();
            // let notify_end_sleep_pair2 = &notify_end_sleep_pair2; // TODO
            let sleep = interruptible_sendable(pause_interrupt_rx.clone(), Arc::new(Mutex::new(Box::pin(async move { // FIXME: locks for too long?
                this2.lock().await.data_mut().pause_interrupt_tx = Some(pause_interrupt_tx);
                sleep(sleep_duration).await;
                notify_end_sleep_pair2.clone().lock().await.0.send(()).await.unwrap();
                Ok::<_, InterruptError>(())
            })))).then(|_| async { () });
            this.lock().await.data_mut().task_queue.lock().await.push_task(Box::pin(sleep)).await;

            let notify_end_sleep_rx = notify_end_sleep_pair.lock().await.1.clone();
            let pause_interrupt_rx = pause_interrupt_rx.clone();
            while this.lock().await.data().pause_interrupt_tx.is_some() {
                select! {
                    _ = async {
                        notify_end_sleep_rx.recv().await.unwrap();
                        this.lock().await.data_mut().pause_interrupt_tx = None;
                    } => { }
                    _ = pause_interrupt_rx.recv() => { } // FIXME: Locks for too long?
                }
            }
        }
    }
    async fn spawn(
        this: Arc<Mutex<Self>>,
        notify_interrupt: async_channel::Receiver<()>,
    ) {
        let task_queue = this.lock().await.data().task_queue.clone();
        TaskQueue::spawn(task_queue, notify_interrupt.clone()); // FIXME: locks too long?
        let this = this;
        let fut = Arc::new(Mutex::new(Box::pin(async move { // FIXME: locks too long?
            Self::_task(this).await;
            Ok::<_, InterruptError>(())
        })));
        // let fut = Arc::new(Mutex::new(fut));
        spawn( interruptible_sendable(notify_interrupt, fut));
    }
    async fn suddenly( this: Arc<Mutex<Self>>) {
        let data = this.lock().await.data(); // FIXME: locks for too long?
        if let Some(ref pause_interrupt_tx) = data.pause_interrupt_tx {
            let fut = pause_interrupt_tx.send(()).await;
            fut.unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::iter::{repeat, repeat_with};
    use std::sync::Arc;
    use std::time::Duration;
    use futures::{Stream, stream};
    use tokio::sync::Mutex;
    use crate::tasks_with_regular_pauses::TasksWithRegularPauses;


    #[test]
    fn empty() {
        let tasks = stream::iter(repeat_with(|| Box::pin(async { () })));
        let tasks = Arc::new(Mutex::new(Box::pin(tasks)));
        let tasks_with_pauses = TasksWithRegularPauses::new(tasks, Duration::from_millis(1));
    }
}