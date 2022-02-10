use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_task_queue::TaskQueue;
// use log::env_logger;

#[tokio::main]
async fn main() {
    env_logger::init();

    let queue = Arc::new(Mutex::new(TaskQueue::new()));
    TaskQueue::push_task(queue.clone(), Box::pin(async { let _ = 1 + 1; })).await;
    let join_handle = TaskQueue::spawn(queue.clone());
    // TaskQueue::push_task(queue.clone(), Box::pin(async { let _ = 2 + 2; })).await;
    queue.lock().await.notify(); // Stop the scheduler.
    join_handle.await.unwrap();
}