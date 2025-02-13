use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use core::{pin::Pin, task::{Poll, Context}};
use futures_util::stream::Stream;
use futures_util::task::AtomicWaker;
use futures_util::stream::StreamExt;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

use crate::print;
use crate::println;

// ArrayQueue::new performs heap allocation. Not possible at compile time.
// cannot initialize static variable directly.
// Use OnceCall type of conquer_once -> makes it possible to perform safe one-time initialization of static values
// initialization doesnt happen in interrupt handler. Prevents the handler from performing heap allocation
static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();

// To implement Waker notif for ScancodeStream, 
// store Waker between poll calls 
// Idea: poll_next stores current waker in this static and add_scancode function calls wake function on it when new scancode is added to queue
static WAKER: AtomicWaker = AtomicWaker::new();

/// Called by the keyboard interrupt handler
/// Must not block or allocate.
/// shouldnt initialize queue in this function since it will be called by interrupt handler (shouldnt perform heap allocation)
pub(crate) fn add_scancode(scancode: u8) 
{
    // try_get: gets reference to initialized queue 
    if let Ok(queue) = SCANCODE_QUEUE.try_get() 
    {
        // case when queue is full
        if let Err(_) = queue.push(scancode) 
        {
            println!("WARNING: scancode queue full; dropping keyboard input");
        }
        else 
        {
            // to wake stored Waker if the push to scancode queue succeeds
            // if waker is registered in WAKER static, method call the equally-named wake method on it
            // and notifies executor.
            WAKER.wake();    

            // important to call wake only after pushing to queue.
            // otherwise, task might be woken too early when queue is still empty
            // can happen when using multi-threaded executor that starts woken task concurrently on diff CPU core
        }
    } 
    else 
    {
        // if queue not initialized, prints warning. no keyboard scancode
        println!("WARNING: scancode queue uninitialized");
    }
}

// To initialize SCANCODE_QUEUE and read scancodes from queue in asynchoronous way
// creates new ScancodeStream type
pub struct ScancodeStream 
{
    _private: (),   // prevents construction of struct from outside of module
                    // new function is the only way to construct the type
}

impl ScancodeStream 
{
    pub fn new() -> Self 
    {
        // initialize SCANCODE_QUEUE static. Panics if already initialized to ensure only single ScancodeStrea instance can be created
        SCANCODE_QUEUE.try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once");
        ScancodeStream { _private: () }
    }
    // to make scancodes availble to asynchronous tasks, need poll like method that tries to pop next scancode off the queue
    // Future isnt a choice: only abstracts over single asynchornous value
}

impl Stream for ScancodeStream 
{
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> 
    {
        // try_get: to get ref to initalized scancode queue
        // queue.pop to try getting next element from queue
        // if fails on first pop call, queue is potentially empty.
        // interrupt handler might have filled queue asynchronously immediately after check
        let queue = SCANCODE_QUEUE.try_get().expect("not initialized");
        
        // fast path
        if let Some(scancode) = queue.pop() 
        {
            return Poll::Ready(Some(scancode));
        }

        // since race condition can occur again, register Waker in WAKER static before second check.
        // wakeup might happen before returning Poll::Pending, but guranteed to get wakeup for any scancode pushed after the check
        WAKER.register(&cx.waker());

        // tries to pop the queue for second time.

        match queue.pop() 
        {
            // if succeeds, return scancode wrapped in Poll::Ready(Some(...))
            // removes registered waker using take because notif is no longer needed
            Some(scancode) => 
            {
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            // if fails, return pending with a registered wakeup
            None => Poll::Pending,
        }
    }
}

// ways for wakeup to happen for task that did not return Poll::Pending yet.
// 1. race condition when wakeup happens immediately before returning Poll::Pending
// 2. queue is no longr empty after registering waker so Poll::Ready is returned
// They arent preventable. executor must handle them

// instead of reading scancode from I/O port, take it from ScancodeStream
pub async fn print_keypresses() 
{
    // Creates Scancode stream then repeatedly use next method provided by StreamExt trait to get Future that resolves to next element in stream
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(ScancodeSet1::new(),
        layouts::Us104Key, HandleControl::Ignore);

    // while let to loop until the stream returns None to signal its 
    // Since poll_next method never returns None, it is an endless loop.
    // print_keypresses task never finishes
    while let Some(scancode) = scancodes.next().await 
    {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) 
        {
            if let Some(key) = keyboard.process_keyevent(key_event) 
            {
                match key 
                {
                    DecodedKey::Unicode(character) => print!("{}", character),
                    DecodedKey::RawKey(key) => print!("{:?}", key),
                }
            }
        }
    }
}
// Keeps CPU busy even if no keys are pressed on keyboard (SimpleExecutor keeps calling pll task in a loop)
// need executor that properly utilizes Waker notification (executor is notified when next keyboard interrupt occurs)
// no need to keep poling print_keypresses task repeatedly