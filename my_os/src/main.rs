#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;

// Overwriting operating system entry point

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> !
{
    loop {}
}