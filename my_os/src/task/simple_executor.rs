use super::Task;
use alloc::collections::VecDeque;
use core::task::{Waker, RawWaker, Context, Poll};
use core::task::RawWakerVTable;

pub struct SimpleExecutor 
{
    // FIFO Queue
    task_queue: VecDeque<Task>,
}

impl SimpleExecutor 
{
    pub fn new() -> SimpleExecutor 
    {
        SimpleExecutor 
        {
            task_queue: VecDeque::new(),
        }
    }

    pub fn spawn(&mut self, task: Task) 
    {
        self.task_queue.push_back(task)
    }

    pub fn run(&mut self) 
    {
        // repeatedly poll all queued task in a loop until all are done
        // but doesnt utilize notifications of Waker type
        while let Some(mut task) = self.task_queue.pop_front() 
        {
            // for each task, creates Context by wrapping a Waker instance returned by dummy_waker
            // then invokes Task::poll with this context. If poll returns Poll::ready, task is finished, move to next.
            // if returns Poll::pending, add back to queue again
            let waker = dummy_waker();
            let mut context = Context::from_waker(&waker);
            match task.poll(&mut context) 
            {
                Poll::Ready(()) => {} // task done
                Poll::Pending => self.task_queue.push_back(task),
            }
        }
    }
}

// To call poll, need Context type which wraps Waker type.
// create dummy waker (does nothing)
fn dummy_raw_waker() -> RawWaker 
{
    // no_op: takes *const () pointer and does nothing
    fn no_op(_: *const ()) {}
    // clone: takes *const () pointer and returns RawWaker by calling itself
    // to clone an operation
    fn clone(_: *const ()) -> RawWaker 
    {
        dummy_raw_waker()
    }

    let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);
    RawWaker::new(0 as *const (), vtable)
}

fn dummy_waker() -> Waker 
{
    // from_raw: unsafe. undefined behavior can occur if programmer doesnt uphold documented requirements of RawWaker
    unsafe { Waker::from_raw(dummy_raw_waker()) }
}

