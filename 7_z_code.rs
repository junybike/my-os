

use pic8259::ChainedPics;
use spin;

// Sets offsets for PICs to range 32 to 47
pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

// Wraps ChainedPics struct in a Mutex. Safe mutable access.
// ChainPics::new function is unsafe since it causes undefined behaviour if PIC is misconfigured
pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

// To enable interrupts...
pub fn init() {
    gdt::init();
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();     // new
}

//==============================
//==============================

// Timer uses line 0 of primary PIC 
// It arrives at CPU as interrupt 32 (0 + offset 32)
// Stores it in InterruptIndex enum

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}
// enum is C-like enum. can directly specify index for each variant

// handler function for the timer interrupt
use crate::print;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        […]
        idt[InterruptIndex::Timer.as_usize()]
            .set_handler_fn(timer_interrupt_handler); // new

        idt
    };
}

// old
// only prints . once
// PIC expects end of interrupt (EOI) from interrupt handler.
// PIC waits for EOI signal before sending the next interrupt
extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    print!(".");
}

// new
// notify_end_of_interrupt figures out whether primary or secondary PIC sent the interrupt
// uses command and data ports to send EOI signal to respective controller
extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    print!(".");

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

//=======================================
//=======================================

// deadlock
// locks WRITER, calls write_fmt, and unlocks it at end of function
// if interrupt occurs while WRITER is locked and interrupt handler tries to print something,
// interrupt handler waits forever 
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
}

// To avoid the deadlock above, disable interrupts as long as Mutex is locked
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;   // new

    // without_interrupts take a closure and execute it in interrupt free enviornment
    interrupts::without_interrupts(|| {     // new
        WRITER.lock().write_fmt(args).unwrap();
    });
}

// Must fix this old test
// test prints string to VGA buffer and checks output manually iterating over the buffer_chars array
// race condition occur because time interrupt handler may run between println and reading of the screen characters
// Old test
#[test_case]
fn test_println_output() {
    let s = "Some test string that fits on a single line";
    println!("{}", s);
    for (i, c) in s.chars().enumerate() {
        let screen_char = WRITER.lock().buffer.chars[BUFFER_HEIGHT - 2][i].read();
        assert_eq!(char::from(screen_char.ascii_character), c);
    }
}

// Lock the WRITER for complete duration of test so that time handler cant write . to screen in between
// new test
#[test_case]
fn test_println_output() {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    let s = "Some test string that fits on a single line";
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writeln!(writer, "\n{}", s).expect("writeln failed");
        for (i, c) in s.chars().enumerate() {
            let screen_char = writer.buffer.chars[BUFFER_HEIGHT - 2][i].read();
            assert_eq!(char::from(screen_char.ascii_character), c);
        }
    });
}

//========================================
//========================================

// hlt instruction
// halts cpu until next interrupt arrives

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

//===========================================
//===========================================

// Keyboard handler

// uses line 1 of primary PIC. arrives at cpu as interrupt 33 (1 + offset 32)

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard, // new
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        […]
        // new
        idt[InterruptIndex::Keyboard.as_usize()]
            .set_handler_fn(keyboard_interrupt_handler);

        idt
    };
}

// When a key is pressed, prints out k
// but only once because it needs to read the scancode of pressed key
extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    print!("k");

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

//=====================================
//=====================================

// Scancode: to find out which key was pressed
// need to query the keyboard controller.
// reads data port of PS/2 controller (the I/O port with number 0x60)

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    use x86_64::instructions::port::Port; 

    // port type of x86_64 crate to read byte from keyboard's data port
    // this byte is scancode and represents key press and releases
    let mut port = Port::new(0x60); 
    let scancode: u8 = unsafe { port.read() };
    print!("{}", scancode);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

//=========================================
//=========================================

// Translates keypresses of number key 0-9 and ignores all other keys
extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    // new
    let key = match scancode {
        0x02 => Some('1'),
        0x03 => Some('2'),
        0x04 => Some('3'),
        0x05 => Some('4'),
        0x06 => Some('5'),
        0x07 => Some('6'),
        0x08 => Some('7'),
        0x09 => Some('8'),
        0x0a => Some('9'),
        0x0b => Some('0'),
        _ => None,
    };
    // destructures the optional key
    // (using shadowing Some(key) to key)
    if let Some(key) = key {
        print!("{}", key);
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

//====================================
//====================================

// use crate: pc-keyboard to translate scancodes of scancode sets 1 and 2

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
    use spin::Mutex;
    use x86_64::instructions::port::Port;

    // creates static keyboard object protected by Mutex
    // initializing keyboard with US keyboard layout and scancode set 1
    // HandleControl allows to map ctrl+[a-z] to Unicode. Ignore those in this handler
    
    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
            Mutex::new(Keyboard::new(ScancodeSet1::new(),
                layouts::Us104Key, HandleControl::Ignore)
            );
    }

    // In each interrupt, lock the Mutex, read scancode from keyboard controller,
    // pass add_byte method to translate scancode into Option<keyEvent>
    // keyEVent contains key which caused the event and whether it was a pressed or released event
    // process_keyevent method translates key event to character
    
    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60);

    let scancode: u8 = unsafe { port.read() };
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => print!("{}", character),
                DecodedKey::RawKey(key) => print!("{:?}", key),
            }
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}