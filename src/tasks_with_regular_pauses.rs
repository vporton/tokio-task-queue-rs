//! Tasks that come separated by time pauses.
//! A task can also be forced to be started at any time, but only during a pause.
//! If a task is forced to be started, the schedule of pauses modifies to accommodate this task.
//!
//! This code is maybe more a demo than a serious module.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::{mpsc, Mutex};
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use async_trait::async_trait;
use tokio_interruptible_future::{InterruptError, interruptible_sendable};

#[allow(dead_code)]
#[derive(Clone)]
pub struct TasksWithRegularPausesData {
    sudden_tx: Arc<Mutex<Option<Sender<()>>>>,
}

impl TasksWithRegularPausesData {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            sudden_tx: Arc::new(Mutex::new(None)),
        }
    }
}

/// See module documentation.
#[async_trait]
pub trait TasksWithRegularPauses<Task: Future<Output = ()> + Send>: Send + Sync + 'static {
    fn data(&self) -> &TasksWithRegularPausesData;
    fn data_mut(&mut self) -> &mut TasksWithRegularPausesData;
    async fn next_task(&self) -> Option<Task>;
    fn sleep_duration(&self) -> Duration;
    async fn _task(&mut self) -> Result<(), InterruptError> { // `InterruptError` here is a hack.
        loop {
            // It is time to run a task.
            // let this1 = this.lock().await;
            let fut = self.next_task().await;
            if let Some(fut) = fut {
                fut.await;
            } else {
                break;
            }

            // Signal that may interrupt the task.
            let (sudden_tx, mut sudden_rx) = mpsc::channel(1);
            self.data_mut().sudden_tx = Arc::new(Mutex::new(Some(sudden_tx)));

            // Re-execute by a signal, or timeout (whichever comes first)
            let sleep_duration = self.sleep_duration(); // lock for one line
            let _ = timeout(sleep_duration, sudden_rx.recv()).await;
        }
        Ok(())
    }
    // FIXME: Moving `self` here is wrong.
    fn spawn(&'static mut self, interrupt_notifier: async_channel::Receiver<()>) -> JoinHandle<Result<(), InterruptError>> {
        spawn( interruptible_sendable(interrupt_notifier, Box::pin(Self::_task(self))))
    }
    async fn suddenly(&self) -> Result<(), tokio::sync::mpsc::error::TrySendError<()>>{
        let sudden_tx = self.data().sudden_tx.lock().await.take(); // atomic operation
        if let Some(sudden_tx) = sudden_tx {
            sudden_tx.try_send(())?;
        }
        Ok(())
    }
}

/// Object-safe variation of TaskQueue
// pub struct ObjectSafeTasksWithRegularPauses<Task: Future<Output = ()> + Send>: Send + Sync + 'static {
//     base: Arc<Mutex<TasksWithRegularPauses<Task>>>,
// }
//
// impl<Task: Future<Output = ()> + Send> ObjectSafeTasksWithRegularPauses<Task> {
//     pub fn new() -> Self {
//         Self {
//             base: Arc::new(Mutex::new(TasksWithRegularPauses::new())),
//         }
//     }
//     pub async fn get_arc(&self) -> &Arc<Mutex<TasksWithRegularPauses>> {
//         &self.base
//     }
//     pub async fn get_arc_mut(&mut self) -> &Arc<Mutex<TasksWithRegularPauses>> {
//         &mut self.base
//     }
// }

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;
    use async_channel::bounded;
    use tokio::sync::Mutex;
    use async_trait::async_trait;
    use tokio::runtime::Runtime;
    use tokio_interruptible_future::InterruptError;
    use crate::TaskItem;
    use crate::tasks_with_regular_pauses::{TasksWithRegularPauses, TasksWithRegularPausesData};

    #[derive(Clone)]
    struct OurTaskQueue {
        data: TasksWithRegularPausesData,
    }

    impl OurTaskQueue {
        pub fn new() -> Self {
            Self {
                data: TasksWithRegularPausesData::new(),
            }
        }
    }

    #[async_trait]
    impl<'a> TasksWithRegularPauses<TaskItem> for OurTaskQueue where Self: 'static {
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
    fn empty() -> Result<(), InterruptError> {
        let queue = OurTaskQueue::new();
        let (interrupt_notifier_tx, interrupt_notifier_rx) = bounded(1);
        let rt  = Runtime::new().unwrap();
        rt.block_on(async {
            OurTaskQueue::spawn(queue.clone(), interrupt_notifier_rx);
            let _ = interrupt_notifier_tx.send(()).await;
            queue.clone().suddenly().await.unwrap();
        });
        Ok(())
    }
}