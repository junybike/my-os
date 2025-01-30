#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(my_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use my_os::println;

// Overwr]iting operating system entry point

#[no_mangle]
pub extern "C" fn _start() -> !
{
    println!("HEllo World{}", "!");
    println!("?????/////");
    
    my_os::init();
    // x86_64::instructions::interrupts::int3();

    fn stackof()
    {
        stackof();
    }
    stackof();

    #[cfg(test)]
    test_main();
    
    println!("running...");
    loop {}
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! 
{
    println!("{}", info);
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! 
{
    my_os::test_panic_handler(info)
}