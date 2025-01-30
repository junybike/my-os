#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use my_os::serial_print;
use core::panic::PanicInfo;
use lazy_static::lazy_static;
use x86_64::structures::idt::InterruptDescriptorTable;
use my_os::{exit_qemu, QemuExitCode, serial_println};
use x86_64::structures::idt::InterruptStackFrame;

#[no_mangle]
pub extern "C" fn _start() -> ! 
{
    serial_print!("stack_overflow::stack_overflow...\t");
    
    my_os::gdt::init(); // initialize new GDT. to register a custom double fault handler that does an exit_qemu instead of panicking
    init_test_idt();
    
    stack_overflow();

    panic!("execution continued after stack overflow");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! 
{
    my_os::test_panic_handler(info)
}

#[allow(unconditional_recursion)]
fn stack_overflow() 
{
    stack_overflow();                   // for each recursion, the return address is pushed
    volatile::Volatile::new(0).read();  // prevent tail recursion optimizations
}

lazy_static! {
    static ref TEST_IDT: InterruptDescriptorTable = 
    {
        let mut idt = InterruptDescriptorTable::new();
        unsafe 
        {
            idt.double_fault
                .set_handler_fn(test_double_fault_handler)
                .set_stack_index(my_os::gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt
    };
}

pub fn init_test_idt() {
    TEST_IDT.load();
}

// When double fault handler is called, exits QEMU with success exit code (test passed)
extern "x86-interrupt" fn test_double_fault_handler(_stack_frame: InterruptStackFrame, _error_code: u64,) -> ! 
{
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}