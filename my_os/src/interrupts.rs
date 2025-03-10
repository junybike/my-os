use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use crate::{gdt, println, print};
use lazy_static::lazy_static;
use pic8259::ChainedPics;   // represents primary/secondary PIC layout
use spin;
use crate::hlt_loop;

// Sets offsets for PICs to range 32 to 47
pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> = 
    spin::Mutex::new(unsafe{ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET)});
// ChainedPics is unsafe since wrong offsets can cause undefined behavior

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex 
{
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex 
{
    fn as_u8(self) -> u8 
    {
        self as u8
    }

    fn as_usize(self) -> usize 
    {
        usize::from(self.as_u8())
    }
}

lazy_static!
{
    static ref IDT: InterruptDescriptorTable = 
    {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        
        unsafe 
        {
            idt.double_fault.set_handler_fn(double_fault_handler)
            .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_interrupt_handler);

        idt
    };
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame)
{
    print!(".");
    unsafe 
    {
        // notify_end_of_interrupt figures out whether primary or secondary PIC sent the interrupt.
        // then uses command and data port to send EOI signal to respective controllers
        // May delete an important unsent interrupt or cause system to hang if wrong interrupt vector number is used
        PICS.lock().
        notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame)
{
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    crate::task::keyboard::add_scancode(scancode); // new

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

extern "x86-interrupt" fn page_fault_handler(stack_frame: InterruptStackFrame, error_code: PageFaultErrorCode)
{
    // CR2 is set by CPU on page fault and contains accesseed virtual address that caused the page fault
    use x86_64::registers::control::Cr2;    
    
    // Cr::read: reads and print the Cr2 info
    // PageFaultErrorCode type provides info about the type of memory access caused the page fault
    // (caused by read or write?)
    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    hlt_loop();
}

pub fn init_idt()
{
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame)
{
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}
extern "x86-interrupt" fn double_fault_handler(stack_frame: InterruptStackFrame, _error_code: u64) -> !
{
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

#[test_case]
fn test_breakpoint_exception()
{
    x86_64::instructions::interrupts::int3();
}