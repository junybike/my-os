#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(my_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use my_os::println;
use bootloader::{BootInfo, entry_point};

// _start is called externally from bootloader. no checking of function signature.
// could take arbitrary args without compilation error and fail or cause undefined behavior at runtime
// entry_point provides type checked way to define Rust function as entry point
entry_point!(kernel_main);

// Overwriting operating system entry point

fn kernel_main(boot_info: &'static BootInfo) -> !
{
    use my_os::memory;
    use x86_64::{structures::paging::Page, VirtAddr};
    use my_os::memory::BootInfoFrameAllocator;

    println!("HEllo World{}", "!");
    my_os::init();
    
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    //let mut frame_allocator = memory::EmptyFrameAllocator;
    let mut frame_allocator = unsafe{BootInfoFrameAllocator::init(&boot_info.memory_map)};

    // map an unused page
    // create mapping for the page at address 0 by calling create_example_mapping with
    // mutable reference to mapper and frame_allocator instances
    // Maps the page to VGA text buffer frame
    // let page = Page::containing_address(VirtAddr::new(0));
    // memory::create_example_mapping(page, &mut mapper, &mut frame_allocator);
    let page = Page::containing_address(VirtAddr::new(0xdeadbeaf000));
    memory::create_example_mapping(page, &mut mapper, &mut frame_allocator);


    // write the string `New!` to the screen through the new mapping
    // Convert page to raw pointer and write a value to offset 400
    let page_ptr: *mut u64 = page.start_address().as_mut_ptr();
    unsafe { page_ptr.offset(400).write_volatile(0x_f021_f077_f065_f04e)};

    #[cfg(test)]
    test_main();
    
    println!("Did not crash :o");
    my_os::hlt_loop();
}

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

