// Future trait
// Output: type of asynchronous value
pub trait Future 
{
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output>;
}

// allows to check if value is already available
// poll method takes two args: self: Pin<&mut Self> and cx: &mut Context
// purpose of cx: &mut Context: to pass a Waker instance to asynchronous task (file system load)
// Waker allows asynchronous task to signal that it is finished
pub enum Poll<T> 
{
    Ready(T),
    Pending,
}

// waiting until future becomes ready
// very inefficient since CPU is busy until value becomes available
// efficient approach: block current thread until future becomes available (need threads)
// systems where blocking is supported: not desired since it turns into synchrnous task
let future = async_read_file("foo.txt");
let file_content = loop 
{
    match future.poll(…) 
    {
        Poll::Ready(value) => break value,
        Poll::Pending => {}, // do nothing
    }
}

//---------------------------------

// string_len function wraps given Future instance to new StringLen struct which implements Future too
// if wrapped future is polled, it polls inner future. if value isnt ready, Poll::Pending is returned from wrapped future as well
// when value is ready, string is extracted from Poll:ready variant and its length is calculated. Then it is wrapped in Poll:Ready again and returned

// Able to calculate length of asynchronous string without waiting for it
// the caller cannot work directly on returned value. It needs to use combinator functions again since function returns a Future
struct StringLen<F> 
{
    inner_future: F,
}

impl<F> Future for StringLen<F> where F: Future<Output = String> 
{
    type Output = usize;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> 
    {
        match self.inner_future.poll(cx) 
        {
            Poll::Ready(s) => Poll::Ready(s.len()),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn string_len(string: impl Future<Output = String>)
    -> impl Future<Output = usize>
{
    StringLen {
        inner_future: string,
    }
}

// Usage
fn file_len() -> impl Future<Output = usize> 
{
    let file_content_future = async_read_file("foo.txt");
    string_len(file_content_future)
}

//---------------------

fn example(min_len: usize) -> impl Future<Output = String> 
{
    // reads foo.txt and use 'then' combinator to chain second future based on the file context
    // if content length is smaller than min_len, read bar.txt and append to content with map combinator
    // otherwise, return only content of foo.txt

    // need move keyword for closure passed to then. otherwise, lifetime err for min_len
    // uses Either wrapper since if and else must have same type. 
    // they try to return different future types in blocks so wrapper type is needed to unify them to single type
    
    // ready function wraps value into future which is immediately ready. needed since Either wrapper expects that wrapped value implements Future
    async_read_file("foo.txt").then(move |content| {
        if content.len() < min_len 
        {
            Either::Left(async_read_file("bar.txt").map(|s| content + &s))
        } 
        else 
        {
            Either::Right(future::ready(content))
        }
    })
}

//------------

async fn foo() -> u32 
{
    0
}

// the above is roughly translated by the compiler to:
fn foo() -> impl Future<Output = u32> 
{
    future::ready(0)
}

// direct translation of example function above that used combinator functions
// async + await
// retrieve values of future without closures or Either types
// state machine:
// (Start) ... waiting on foo.txt ... waiting on bar.txt ... (End)
// waiting on foo.txt: waiting for async_read_file("foo.txt")
async fn example(min_len: usize) -> String 
{
    let content = async_read_file("foo.txt").await;
    if content.len() < min_len 
    {
        content + &async_read_file("bar.txt").await
    } 
    else 
    {
        content
    }
}
// Compiler generates...
// at Start and Waiting on foo.txt states, min_len parameter must be stored for comparison with content.len()
// waiting on foo txt state also stores foo_txt_future which represents future returned by async_read_file call
// - this future must be polled again when state machine continues. so it must be saved
struct StartState 
{
    min_len: usize,
}

struct WaitingOnFooTxtState 
{
    min_len: usize,
    foo_txt_future: impl Future<Output = String>,
}
// Waiting on bar txt state contains content variable for later string concatenation when bar.txt is ready
// bar_txt_future represents in-progress load of bar.txt.
// no min_len since no need after content.len() comparision
struct WaitingOnBarTxtState 
{
    content: String,
    bar_txt_future: impl Future<Output = String>,
}

struct EndState {}

// to create a state machine, combine them to enum
enum ExampleStateMachine 
{
    Start(StartState),
    WaitingOnFooTxt(WaitingOnFooTxtState),
    WaitingOnBarTxt(WaitingOnBarTxtState),
    End(EndState),
}
// defined separate enum variant for each state and add corresponding state struct to each variant as field
// compiler implementing state transition:
impl Future for ExampleStateMachine 
{
    type Output = String; // return type of `example`

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> 
    { 
        // for poll function, use match statement inside a loop
        // switch to next state as long as possible and use explicit return Poll::Pending when we cant continue
        loop 
        {
            
            match self 
            { // TODO: handle pinning
                ExampleStateMachine::Start(state) => {…}
                ExampleStateMachine::WaitingOnFooTxt(state) => {…}
                ExampleStateMachine::WaitingOnBarTxt(state) => {…}
                ExampleStateMachine::End(state) => {…}
            }
        }
    }
}
// Output type of future is String because it is the return tyoe of example function

// the start state
// executes from body of example function until the first .await
// to handle .await operation, change the state of self state machine to waiting on foo txt
ExampleStateMachine::Start(state) => 
{
    // from body of `example`
    let foo_txt_future = async_read_file("foo.txt");
    // `.await` operation
    let state = WaitingOnFooTxtState 
    {
        min_len: state.min_len,
        foo_txt_future,
    };
    *self = ExampleStateMachine::WaitingOnFooTxt(state);
}

ExampleStateMachine::WaitingOnFooTxt(state) => 
{
    // first calls poll function of foo_txt_future.
    // if its not ready, exit loop and return Poll::pending
    // since self stays in WaitingOnFooTxt state in this case, 
    // next poll call on state machine will enter same match arm and retry polling foo_txt_future
    
    // when foo_txt_future is ready, assign result to content variable and continue to execute code of example function 
    // - if content.len() is smaller than min_len saved in state struct, bar.txt is read asynchornously

    // then again translate .await for WaitingOnBarTxt state
    // since executing match inside a loop, execution jumps to match arm for new state afterward where bar_txt_future is polled
    // else branch -> no .await occurs. reached to end of function and return content wrapped in Poll::Ready. Changes current state to End
    match state.foo_txt_future.poll(cx) 
    {
        Poll::Pending => return Poll::Pending,
        Poll::Ready(content) => 
        {
            // from body of `example`
            if content.len() < state.min_len 
            {
                let bar_txt_future = async_read_file("bar.txt");
                // `.await` operation
                let state = WaitingOnBarTxtState 
                {
                    content,
                    bar_txt_future,
                };
                *self = ExampleStateMachine::WaitingOnBarTxt(state);
            } 
            else 
            {
                *self = ExampleStateMachine::End(EndState);
                return Poll::Ready(content);
            }
        }
    }
}
// Waiting on bar txt state
ExampleStateMachine::WaitingOnBarTxt(state) => 
{
    // start by polling bar_txt_future. If pending, exit loop and return Poll::Pending
    // otherwise, perform concatenation (last operation of example function)
    // then updates state machine to End state and return result wrapped in Poll::Ready
    match state.bar_txt_future.poll(cx) 
    {
        Poll::Pending => return Poll::Pending,
        Poll::Ready(bar_txt) => 
        {
            *self = ExampleStateMachine::End(EndState);
            // from body of `example`
            return Poll::Ready(state.content + &bar_txt);
        }
    }
}
// End state
ExampleStateMachine::End(_) => 
{
    // Futures shouldnt be polled again after returned Poll::Ready.
    // panics if poll is called in End state
    panic!("poll called after Poll::Ready was returned");
}

//----------
async fn example(min_len: usize) -> String

// Generated code:
fn example(min_len: usize) -> ExampleStateMachine 
{
    // initializes state machine and return it
    // no longer has async since it explicitly returns ExampleStateMachine type which implements Future trait
    ExampleStateMachine::Start(StartState {
        min_len,
    })
}

//-----------------------

// creates array with contents 1 2 3. 
// then creates reference to last array element and stores it in element variable.
// then asynchronously writes number converted to string to foo.txt file
// returns number referenced by element
async fn pin_example() -> i32 
{
    let array = [1, 2, 3];
    let element = &array[2];
    async_write_file("foo.txt", element.to_string()).await;
    *element
}
// States: start, end, waiting on write
// function takes no arg -> start state's struct is empty

// waiting on write state
// element is required for return value and array is referenced by element.
// element: last element of array field (depends on where struct lives in memory)
// Self referential structs: structs with internal pointers (reference themselves from one of their fields)
struct WaitingOnWriteState 
{
    array: [1, 2, 3],
    element: 0x1001c, // address of the last array element
}

//---------------------

fn main() 
{
    // initializing the struct with null pointer and allocate it on heap using Box::new
    let mut heap_value = Box::new(SelfReferential 
    {
        self_ptr: 0 as *const _,
    });

    // determine memory address of heap allocated struct and store into ptr variable
    let ptr = &*heap_value as *const SelfReferential;

    // assign ptr variable to self_ptr field
    heap_value.self_ptr = ptr;
    println!("heap value at: {:p}", heap_value);
    println!("internal reference: {:p}", heap_value.self_ptr);
}

struct SelfReferential 
{
    self_ptr: *const Self,
}

// Breaking the code example: move out of Box<T> or replace its content
// use mem::replace to replace heap allocated value with new struct instance
// it moves original heap_value to stack and self_ptr is now a dangling pointer pointing to old heap address
// cause: Box<T> allows us to get &mut T ref to heap allocated value
// fix: prevent &mut ref to self referential structs from being created
let stack_value = mem::replace(&mut *heap_value, SelfReferential 
{
    self_ptr: 0 as *const _,
});
println!("value at: {:p}", &stack_value);
println!("internal reference: {:p}", stack_value.self_ptr);

// updating SelfReferential type to opt out of Unpin
use core::marker::PhantomPinned;

struct SelfReferential 
{
    self_ptr: *const Self,
    _pin: PhantomPinned,    // zero sized marker. a single field that is not Unpin makes complete struct opt out of Unpin
}

// change the Box<SelfReferential> type in example to Pin<Box<SelfReferential>>
// uses Box::pin instead of Box::new
let mut heap_value = Box::pin(SelfReferential 
{
    self_ptr: 0 as *const _,
    _pin: PhantomPinned,
});

// Since compiler cant detect between valid and invalid uses of &mut ref, use get_unchecked_mut
unsafe 
{
    // get_unchecked_mut works on Pin<&mut T> instead of Pin<Box<T>>. convert by using Pin::as_mut
    // then set self_ptr using &mut ref returned by get_unchecked_mut
    let mut_ref = Pin::as_mut(&mut heap_value);
    Pin::get_unchecked_mut(mut_ref).self_ptr = ptr;
}

//-----------------

// takes self: Pin<&mut Self> instead of normal &mut self
// future instances created from async/await are often self-referential
// by wrapping self to Pin and let compiler optout of Unpin for self referential futures generated from async/await,
// guranteed that futures are not moved memory between poll calls
// ensures all internal references are still valid
fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output>

// at start state, only contains function args. no internal ref
// to call poll, caller must wrap future into Pin first (no moving in memory)

//----------------

// Hard disk driver internally store Waker passed to poll call and use it to notify executor when file is written to disk
// so executor no need to waste time trying to poll future again before it receives waker notification
async fn write_file() {
    async_write_file("foo.txt", "Hello").await;
}

// -----------------------

// compiler will transform it into state machine that implements Future
// Future will return Poll::Ready(42) on first poll call
async fn async_number() -> u32
{
    42
}
// to run future returned by example_task, need to call poll on it until signals its completion by returning Poll::Ready
// must create executor type
// this function dont return anything. prints something to screen as side effect
async fn example_task()
{
    let number = async_number().await();
    println!("async number: {}", number);
}

//---------------------

impl Stream for ScancodeStream 
{
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> 
    {
        // try_get: to get ref to initalized scancode queue
        // queue.pop to try getting next element from queue
        // if succeeds, return scancode wrapped in Poll::Ready(Some(...))
        // if fails, queue is empty. returns Poll::Pending
        let queue = SCANCODE_QUEUE.try_get().expect("not initialized");
        match queue.pop() {
            Some(scancode) => Poll::Ready(Some(scancode)),
            None => Poll::Pending,
        }
    }
}

//-------------------------

// instead of poll returning Poll<Self::Item>, Stream trait defines poll_next returning Poll<Option<Self::Item>>
// poll_next can be called repeatedly until it returns Poll::Ready(None) to signal that stream is finished
// similar to Iterator::next method 
pub trait Stream 
{
    type Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context)
        -> Poll<Option<Self::Item>>;
}
// poll_next requires asynchronous task to notify executor when it becomes ready after Poll::Pending is returned
// so executor doesnt need to poll same task again until it is notified (reduces perforance overhead of waiting tasks)
// to send notif, extract Waker from passed Context ref and store it somewhere
// when task becomes ready, invoke wake method on stored Waker to notify executor that task should be polled again
// AtomicWaker

//----------------------

    // cretes SimpleExecutor with empty task_queue
    // calls asynchronous example task function (returns future)
    // wraps future in Task type which moves it to heap and pins it
    // then adds task to task_queue of executor through spawn method
    let mut executor = SimpleExecutor::new();
    executor.spawn(Task::new(example_task()));
    
    // when runs, 
    // pops task from front of task_queue
    // creates RawWaker to task, convert it to Waker, then create Context instance from it
    // calling poll method on future of task using Context we created
    // example_task doesnt wait for anything. directly run until its end on first poll call (prints async number 42)
    // since it directly returns Poll::Ready, not added back to task queue
    executor.run();
    // returns after task_queue becomes empty

//--------------------

//
fn sleep_if_idle(&self)
{
    if self.task_queue.is_empty()
    {
        // interrupt may happen before hlt.
        // race condition possibilitiy
        // interrupt pushes to task_queue. (putting CPU to sleep even if there is a ready task)
        // may delay handling of keyboard interrupt until next keypress or next timer interrupt
        // Fix:
        // disable interrupt on CPU before check and atomically enable again together with hlt instruction           
        x86_64::instructions::hlt();
    }
}