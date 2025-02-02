use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::println;

pub fn init_idt() {
    // pause a program when int3 is executed. used in debuggers
    let mut idt = InterruptDescriptorTable::new(); 
    idt.breakpoint.set_handler_fn(breakpoint_handler);
}

// prints a message when breakpoint instruction is executed
extern "x86-interrupt" fn breakpoint_handler(
    stack_frame: InterruptStackFrame)
{
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

//===================================
//===================================

// for CPU to use new interrupt descriptor table, load it using lidt instruction

// InterruptDescriptorTable struct of x86_64 provides load method

// Old version
pub fn init_idt() {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.load();
}

// load method expects &'static self (reference valid for complete runtime of program)
// CPU accesses this table every interrupt until we load a different IDT
// using a shorter lifetime than 'static can lead to use after free bug

// using static mut is very prone to data races. requires unsafe block
// Bad solution
static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn init_idt() {
    unsafe {
        IDT.breakpoint.set_handler_fn(breakpoint_handler);
        IDT.load();
    }
}

// need to store idt at a place where it has a 'static lifetime
// the macro performs initialization when static is reference the first time

// New version
use lazy_static::lazy_static;
lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}