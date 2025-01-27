#![no_std]
#![no_main]

use core::panic::PanicInfo;
use my_os::{exit_qemu, serial_print, serial_println, QemuExitCode};

// For integration tests with a single test function, test runner isnt necessary
// Disable the test runner and run test directly in _start function

// Calls should_fail directly from _start function and exit with a failure exit code if it returns

#[no_mangle]
pub extern "C" fn _start() -> ! {
    should_fail();
    serial_println!("[test didnt panic]");
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! 
{
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}