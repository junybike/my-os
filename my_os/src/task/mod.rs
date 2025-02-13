use core::{future::Future, pin::Pin};
use alloc::boxed::Box;
use core::task::{Context, Poll};
use core::sync::atomic::{AtomicU64, Ordering};

pub mod simple_executor;
pub mod keyboard;
pub mod executor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TaskId(u64); // simple wrapper type around u64. 
// derives a number of traits for it to make it printable, copyable, comparable, and sortable

impl TaskId 
{
    // To create a new unique ID
    fn new() -> Self 
    {
        // NEXT_ID variable of type Atomic64 to ensure each ID is assigned only once
        // fetch_add method automatically increases value and returns previous value in one atomic operation
        // when TaskId::new is called in parallel, every ID is returned exactly once
        // Ordering defines whether compiler is allowed to reorder fetch_add operation in instructions stream
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

// newtype wrapper around pinned, heap allocated and dynamically dispatched future with empty type () as output
pub struct Task 
{
    // requires that future associated with task returns ().
    // tasks dont return any result. just exected for their side effects
    // dyn: indicates that we store trait object in Box (methods on future are dynamically dispatched. allows diff types of futures to be stored in Task type)
    // Pin<Box> ensures value cannot be moved in memory by placing it on heap and prevents creation of &mut ref to it
    future: Pin<Box<dyn Future<Output = ()>>>,
    
    // id field makes it possible to uniquely name a task
    // required for waking a specific task
    id: TaskId,
}
impl Task 
{
    // takes arbitrary future with output type ().
    // pins it in memory through Box::pin function
    // then wraps boxed future in Task struct and returns it.
    // static lifetime because returned Task can live for arbitrary time
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task 
    {
        Task 
        {
            future: Box::pin(future),
            id: TaskId::new(),
        }
    }

    // poll method on Future trait expect toe be called on Pin<&mut T>.
    // must use Pin::as_mut to convert self.future field of type Pin<Box<T>>.
    // then call poll on converted self.future and returns result
    fn poll(&mut self, context: &mut Context) -> Poll<()> 
    {
        self.future.as_mut().poll(context)
    }

}