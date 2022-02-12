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
use futures::{ready, Stream, StreamExt, TryFutureExt};
use tokio::sync::Notify;
use tokio::time::{Sleep, sleep};
use tokio_interruptible_future::{InterruptError, interruptible};
use crate::{TaskItem, TaskQueue};

struct TasksWithRegularPauses {
    // we_are_in_pause: bool,
    task_queue: TaskQueue,
    pause: Option<Fuse<Sleep>>,
    pause_interrupt: Arc<Notify>,
    forced: bool,
    sleep_duration: Duration, // TODO: Should be a method.
}

// FIXME: Correct?
impl Unpin for TasksWithRegularPauses { }

impl TasksWithRegularPauses {
    pub fn new(sleep_duration: Duration) -> Self {
        Self {
            task_queue: TaskQueue::new(),
            // we_are_in_pause: false,
            pause: None,
            pause_interrupt: Arc::new(Notify::new()),
            forced: false,
            sleep_duration, // TODO: Should be a method.
        }
    }
    pub fn spawn() {

    }
}

impl for TasksWithRegularPauses<TaskCreator> {
    type Item = TaskItem;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>
    ) -> Poll<Option<Self::Item>> {
        if let Some(pause) = &self.pause {
            if self.forced {
                self.pause_interrupt.notify_one();
                self.forced = false;
            }
            Pin::new(&mut self.task_creator).poll_next(cx)
            // if let Poll::Ready(task) = self.task_creator.poll_next(cx) {
            //
            // }
        } else {
            self.get_mut().pause_interrupt = Arc::new(Notify::new());
            use futures::future::FutureExt;
            let sleep_duration = self.sleep_duration;
            Poll::Ready(
                Some(
                    Box::pin(
                    Box::pin(
                        interruptible(
                            self.pause_interrupt.clone(),
                            async move {
                                sleep(sleep_duration).await;
                                Ok(())
                            }
                        ).then(|_: Result<_, InterruptError>| ready(()))
                    )
                )
                )
            )
        }
    }
}
