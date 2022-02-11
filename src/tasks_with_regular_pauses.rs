//! Tasks that come separated by time pauses.
//! A task can also be forced to be started at any time, but only during a pause.
//! If a task is forced to be started, the schedule of pauses modifies to accomodate this task.
//!
//! This code is more a demo of my `tokio-task-queue` than a serious module.

use std::future::Future;
use std::ops::Not;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use futures::future::{Fuse, FusedFuture, ready};
use futures::{ready, Stream, StreamExt, TryFutureExt};
use tokio::sync::Notify;
use tokio::time::{Sleep, sleep};
use tokio_interruptible_future::{InterruptError, interruptible};
use crate::TaskItem;

struct TasksWithRegularPausesStream<TaskCreator: Stream<Item = TaskItem> + Sync + Unpin> {
    // we_are_in_pause: bool,
    task_creator: TaskCreator,
    pause: Option<Fuse<Sleep>>,
    pause_interrupt: Arc<Notify>,
    forced: bool,
    sleep_duration: Duration, // TODO: Should be a method.
}

impl<TaskCreator: Stream<Item = TaskItem> + Sync + Unpin> TasksWithRegularPausesStream<TaskCreator> {
    pub fn new(task_creator: TaskCreator, sleep_duration: Duration) -> Self {
        Self {
            task_creator,
            // we_are_in_pause: false,
            pause: None,
            pause_interrupt: Arc::new(Notify::new()),
            forced: false,
            sleep_duration, // TODO: Should be a method.
        }
    }
}

impl<TaskCreator: Stream<Item = TaskItem> + Sync + Unpin> Stream for TasksWithRegularPausesStream<TaskCreator> {
    type Item = TaskItem;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>
    ) -> Poll<Option<Self::Item>> {
        if let Some(pause) = self.pause {
            if self.forced {
                self.pause_interrupt.notify_one();
                self.forced = false;
            }
            Pin::new(&mut self.task_creator).poll_next(cx)
            // if let Poll::Ready(task) = self.task_creator.poll_next(cx) {
            //
            // }
        } else {
            self.pause_interrupt = Arc::new(Notify::new());
            use futures::future::FutureExt;
            Poll::Ready(
                Some(
                    Box::pin(
                    Box::pin(
                        interruptible(
                            self.pause_interrupt.clone(),
                            async { cx.waker().wake() }
                                .then(|_| sleep(self.sleep_duration))
                                .then(|_| ready(Ok(())))
                            // async move {
                            //     sleep(self.sleep_duration).await;
                            //     cx.waker().wake();
                            //     Ok(())
                            // }
                        ).then(|_: Result<_, InterruptError>| ready(()))
                    )
                )
                )
            )
        }
    }
}

pub struct TasksWithRegularPauses {

}