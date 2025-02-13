use super::{Task, TaskId};
use alloc::{collections::BTreeMap, sync::Arc};
use core::task::Waker;
use crossbeam_queue::ArrayQueue;
use core::task::{Context, Poll};
use alloc::task::Wake;

pub struct Executor 
{
    // map is indexed by TaskID to allow efficient continuation of specific task
    tasks: BTreeMap<TaskId, Task>,          // contains actual Task instances
    task_queue: Arc<ArrayQueue<TaskId>>,    // array queue of taskIDs wrapped into Arc type (implements ref counting)
                                            // ref counting makes it possible to share ownership among multiple owners
                                            // allocates value on heap and counting number of active ref to it
    waker_cache: BTreeMap<TaskId, Waker>,   // caches Waker of a task after its creation
                                            // improves performance by reusing same waker multiple wake ups of same task instead of creating new waker each time
                                            // ensures ref counted wakers are not deallocated inside interrupt handlers which may lead to deadlocks
}
// Arc<ArrayQueue> because it is shared with executor and wakers
// waker pushes ID of woken task to queue -> executor sits on receiveing end of queue -> retrieves woken tasks by their ID from tasks map and runs them

impl Executor 
{
    pub fn new() -> Self 
    {
        Executor 
        {
            tasks: BTreeMap::new(),
            task_queue: Arc::new(ArrayQueue::new(100)),
            waker_cache: BTreeMap::new(),
        }
    }

    // adds a given task to tasks map and immediately wakes it by pushing its ID to task_queue
    pub fn spawn(&mut self, task: Task)
    {
        let task_id = task.id;

        // if task with same id is in map,  
        if self.tasks.insert(task.id, task).is_some() 
        {
            panic!("task with same ID already in tasks");
        }
        self.task_queue.push(task_id).expect("queue full");
    }

    // to execute all tasks in task_queue
    // loops over all tasks in task_queue, create waker for each task, poll them.
    // instead of adding pending task back to queue, TaskWaker take care of adding woken tasks back to queue
    fn run_ready_tasks(&mut self) 
    {
        // destructure `self` to avoid borrow checker errors
        let Self 
        {
            tasks,
            task_queue,
            waker_cache,
        } = self;

        // for each popped task id, retrieve mutable ref to corresponding task from tasks map
        while let Some(task_id) = task_queue.pop() 
        {
            let task = match tasks.get_mut(&task_id) 
            {
                Some(task) => task,
                None => continue, // task no longer exists
            };

            // to avoid performance overhead of creating waker on each poll, 
            // use waker_cache map to store waker for each task after it has been created
            // or_insert_with to create new waker if it doesnt exists yet then get mutable reference to it
            let waker = waker_cache
                .entry(task_id)
                .or_insert_with(|| TaskWaker::new(task_id, task_queue.clone()));
            let mut context = Context::from_waker(waker);
            
            match task.poll(&mut context) 
            {
                // task is finished when it returns Ready
                Poll::Ready(()) => 
                {
                    // task done -> remove it and its cached waker
                    tasks.remove(&task_id);
                    waker_cache.remove(&task_id);
                }
                Poll::Pending => {}
            }
        }
    }
    pub fn run(&mut self) -> ! 
    {
        loop 
        {
            self.run_ready_tasks();
            self.sleep_if_idle();   // when queue is empty
        }
    }    
    fn sleep_if_idle(&self)
    {
        use x86_64::instructions::interrupts::{self, enable_and_hlt};

        // disable interrupt before checking whether task_queue is empty.
        interrupts::disable();
        if self.task_queue.is_empty() 
        {
            // enables interrupts and put CPU to sleep as a single atomic operation
            enable_and_hlt();
        } 
        else 
        {
            // interrupt woke a task after run_ready_tasks returned
            // enable interrupt again and continue execution without hlt
            interrupts::enable();
        }
    }
}

struct TaskWaker // since ownership of task_queue is shared, use Arc to implement shared ref counted ownership
{
    task_id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
}
impl TaskWaker 
{
    fn wake_task(&self) 
    {
        // pushing task_id to reference task_queue
        self.task_queue.push(self.task_id).expect("task_queue full");
    }
    fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker 
    {
        // from method takes care of constructing RawWakerVTable and RawWaker for TaskWaker
        Waker::from(Arc::new(TaskWaker 
        {
            task_id,
            task_queue,
        }))
    }
}

// to use TaskWaker for polling futures, must convert it to Waker instance.
impl Wake for TaskWaker 
{
    // takes ownership of Arc (rquires increase of ref count)
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }
    // takes ref to Arc   
    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}