#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(my_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::panic::PanicInfo;
use my_os::println;
use bootloader::{BootInfo, entry_point};
use alloc::{boxed::Box, vec, vec::Vec, rc::Rc};

// _start is called externally from bootloader. no checking of function signature.
// could take arbitrary args without compilation error and fail or cause undefined behavior at runtime
// entry_point provides type checked way to define Rust function as entry point
entry_point!(kernel_main);

// Overwriting operating system entry point

fn kernel_main(boot_info: &'static BootInfo) -> !
{
    use my_os::allocator;
    use my_os::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    println!("Hellow world{}", "!");
    my_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };

    // new
    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    // allocate a number on the heap
    let heap_value = Box::new(41);
    println!("heap_value at {:p}", heap_value);

    // create a dynamically sized vector
    let mut vec = Vec::new();
    for i in 0..500 
    {
        vec.push(i);
    }
    // prints underlying heap pointers using {:p} formatting specifier
    println!("vec at {:p}", vec.as_slice());

    // create a reference counted vector -> will be freed when count reaches 0
    let reference_counted = Rc::new(vec![1, 2, 3]);
    let cloned_reference = reference_counted.clone();
    println!("current reference count is {}", Rc::strong_count(&cloned_reference));
    core::mem::drop(reference_counted);
    println!("reference count is {} now", Rc::strong_count(&cloned_reference));

    #[cfg(test)]
    test_main();
    
    println!("Did not crash :o");
    my_os::hlt_loop();
}

// Starts at 0x4444_4444_* prefix
// Reason why vector starts at offset 0x800 is not because boxed value is 0x800 bytes large
// Its that reallocations occur when vector needs to increase its capacity
// Vector allocates new backing array with larger capacity and copies all elements over. Frees the old allocation

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! 
{
    println!("{}", info);
    my_os::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! 
{
    my_os::test_panic_handler(info)
}

