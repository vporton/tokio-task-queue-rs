//! Tasks that come separated by time pauses.
//! A task can also be forced to be started at any time, but only during a pause.
//! If a task is forced to be started, the schedule of pauses modifies to accommodate this task.
//!
//! This code is maybe more a demo than a serious module.

use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::{mpsc, Mutex};
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use async_trait::async_trait;
use tokio_interruptible_future::{InterruptError, interruptible_sendable};
use crate::TaskItem;

#[allow(dead_code)]
pub struct TasksWithRegularPausesData {
    sudden_tx: Option<Arc<Mutex<Sender<()>>>>,
}

impl TasksWithRegularPausesData {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            sudden_tx: None,
        }
    }
}

/// See module documentation.
#[async_trait]
pub trait TasksWithRegularPauses: Send + Sync + 'static {
    fn data(&self) -> &TasksWithRegularPausesData;
    fn data_mut(&mut self) -> &mut TasksWithRegularPausesData;
    async fn next_task(&self) -> Option<TaskItem>;
    fn sleep_duration(&self) -> Duration;
    async fn _task(this: Arc<Mutex<Self>>) -> Result<(), InterruptError> { // `InterruptError` here is a hack.
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
            let sleep_duration = this.lock().await.sleep_duration(); // lock for one line
            let _ = timeout(sleep_duration, sudden_rx.recv()).await;
        }
        Ok(())
    }
    fn spawn(this: Arc<Mutex<Self>>, interrupt_notifier: tokio_interruptible_future::Receiver<()>) -> JoinHandle<Result<(), InterruptError>> {
        spawn( interruptible_sendable(interrupt_notifier, Arc::new(Mutex::new(Box::pin(Self::_task(this))))))
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
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use async_trait::async_trait;
    use tokio::runtime::Runtime;
    use tokio_interruptible_future::{InterruptError, broadcast};
    use crate::TaskItem;
    use crate::tasks_with_regular_pauses::{TasksWithRegularPauses, TasksWithRegularPausesData};

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
    fn empty() -> Result<(), InterruptError> {
        let queue = Arc::new(Mutex::new(OurTaskQueue::new()));
        let (interrupt_notifier_tx, interrupt_notifier_rx) = broadcast::<()>(1);
        let rt  = Runtime::new().unwrap();
        rt.block_on(async {
            OurTaskQueue::spawn(queue, interrupt_notifier_rx);
            let _ = interrupt_notifier_tx.send(()).await;
        });
        Ok(())
    }
}