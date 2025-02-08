#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(my_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use alloc::boxed::Box;
use alloc::vec::Vec;
use my_os::allocator::HEAP_SIZE;

entry_point!(main);

#[panic_handler]
fn panic(info: &PanicInfo) -> ! 
{
    my_os::test_panic_handler(info)
}

fn main(boot_info: &'static BootInfo) -> ! 
{
    use my_os::allocator;
    use my_os::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    my_os::init();
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe 
    {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };
    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    test_main();
    loop{}
}

// Basic allocation test
// performs simple allocations using Box and checks allocated values 
// Verifies that no allocation error occurs
#[test_case]
fn simple_allocation() 
{
    let heap_value_1 = Box::new(41);
    let heap_value_2 = Box::new(13);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);
}

// Tests large allocations and multiple allocations (Reallocations)
#[test_case]
fn large_vec() 
{
    let n = 1000;
    let mut vec = Vec::new();
    for i in 0..n 
    {
        vec.push(i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
}

// Creates 10k allocations after each other
// ensures that allocator reuses freed memory for subsequent allocations
#[test_case]
fn many_boxes() 
{
    for i in 0..HEAP_SIZE 
    {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
}

// long_lived allocation lives for whole loop execution
// creates long_lived allocation at first (start of heap)
// in each iteration, short lived allocation is created and freed again before next iteration
// (counter is 2 at the beginning of iteration and decrease to 1 at the end)
// In bump allocator, counter does not fall to 0 before the end of the loop
#[test_case]
fn many_boxes_long_lived() {
    let long_lived = Box::new(1); // new
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
    assert_eq!(*long_lived, 1); // new
}