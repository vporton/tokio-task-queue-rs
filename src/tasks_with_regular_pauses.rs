//! Tasks that come separated by time pauses.
//! A task can also be forced to be started at any time, but only during a pause.
//! If a task is forced to be started, the schedule of pauses modifies to accomodate this task.
//!
//! This code is more a demo of my `tokio-task-queue` than a serious module.

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use futures::future::{Fuse, FusedFuture, ready};
use futures::{ready, SinkExt, Stream, StreamExt, TryFutureExt};
use futures::FutureExt;
use tokio::{select, spawn};
use tokio::sync::{Mutex, Notify};
use tokio::sync::mpsc::channel;
use tokio::task::JoinHandle;
use tokio::time::{Sleep, sleep};
use tokio_interruptible_future::{InterruptError, interruptible};
use crate::{TaskItem, TaskQueue};

// TODO: Eliminate `'static`.
pub struct TasksWithRegularPauses<Tasks: 'static + Stream<Item = TaskItem> + Send + Unpin> {
    tasks: Tasks,
    // we_are_in_pause: bool,
    task_queue: Arc<Mutex<TaskQueue>>,
    pause_interrupt_tx: Option<async_channel::Sender<()>>, // `None` when not in pause // TODO: `Notify` instead?
    sleep_duration: Duration, // TODO: Should be a method.
}

// FIXME: Correct?
// impl Unpin for TasksWithRegularPauses { }

impl<Tasks: 'static + Stream<Item = TaskItem> + Send + Unpin> TasksWithRegularPauses<Tasks> {
    pub fn new(tasks: Tasks, sleep_duration: Duration) -> Self {
        Self {
            tasks,
            task_queue: Arc::new(Mutex::new(TaskQueue::new())),
            // we_are_in_pause: false,
            // pause: None,
            pause_interrupt_tx: None,
            sleep_duration, // TODO: Should be a method.
        }
    }
    async fn _task(this: Arc<Mutex<Self>>) {
        let this2 = this.clone();
        loop {
            { // block
                let mut this1 = this.lock().await;
                if let Some(task) = this1.tasks.next().await {
                    this1.task_queue.lock().await.push_task(Box::pin(task)).await;
                } else {
                    break;
                }
            }

            let sleep_duration = this.lock().await.sleep_duration;
            let (pause_interrupt_tx, pause_interrupt_rx) = async_channel::bounded(1);
            let (notify_end_sleep_tx, notify_end_sleep_rx) = async_channel::bounded(1);
            let this2 = this2.clone();
            let sleep = interruptible(pause_interrupt_rx.clone(), async move { // FIXME: locks for too long?
                this2.lock().await.pause_interrupt_tx = Some(pause_interrupt_tx);
                sleep(sleep_duration).await;
                notify_end_sleep_tx.send(()).await.unwrap();
                Ok::<_, InterruptError>(())
            }).then(|_| async { () });
            // let sleep = sleep;
            this.lock().await.task_queue.lock().await.push_task(Box::pin(sleep)).await;

            let notify_end_sleep_rx = notify_end_sleep_rx.clone();
            let pause_interrupt_rx = pause_interrupt_rx.clone();
            while this.lock().await.pause_interrupt_tx.is_some() {
                select! {
                    _ = async {
                        notify_end_sleep_rx.recv().await.unwrap();
                        this.lock().await.pause_interrupt_tx = None;
                    } => { }
                    _ = pause_interrupt_rx.recv() => { } // FIXME: Locks for too long?
                }
            }
        }
    }
    pub async fn spawn(
        this: Arc<Mutex<Self>>,
        notify_interrupt: async_channel::Receiver<()>,
    ) {
        let task_queue= this.lock().await.task_queue.clone();
        TaskQueue::spawn(task_queue, notify_interrupt.clone()); // FIXME: locks too long?
        spawn( interruptible(notify_interrupt, async move { // FIXME: locks too long?
            Self::_task(this).await;
            Ok::<_, InterruptError>(())
        }));
    }
    pub async fn suddenly( this: Arc<Mutex<Self>>) {
        if let Some(ref pause_interrupt_tx) = this.lock().await.pause_interrupt_tx {
            pause_interrupt_tx.send(()).await.unwrap();
        }
    }
}
