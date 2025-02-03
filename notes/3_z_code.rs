#[allow(dead_code)] // allow unused variant
#[derive(Debug, Clone, Copy, PartialEq, Eq)] // derives the essential traits
#[repr(u8)] // each enum variant is stored as u8
pub enum Color {
    Black = 0,
    // ...
    White = 15,
}

//==================================================
//==================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)] // to ensure that ColorCode has exact same data layout as u8
struct ColorCode(u8); // contain foreground and background color

impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

//==================================================
//==================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)] // gurantees that struct's fields are laid out like C struct. gurantees correct field ordering
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

#[repr(transparent)] // ensures that it has same memory layout as its single field
struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

//==================================================
//==================================================

// Writer type: to write to screen
pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
}

impl Writer {
    // Writing a single ASCII byte
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            // if value is the newline byte, writer calls new_line method
            b'\n' => self.new_line(), 
            // other bytes get printed to screen 
            byte => {
                // if current line is full, new_line is called 
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;
                self.buffer.chars[row][col] = ScreenChar {
                    ascii_character: byte,
                    color_code,
                };
                self.column_position += 1;
            }
        }
    }
    // Printing the whole string: convert them to bytes and print them one by one
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // printable ASCII byte or newline
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // not part of printable ASCII range
                _ => self.write_byte(0xfe),
            }

        }
    }

    fn new_line(&mut self) {/* TODO */}
}

pub fn print_something() {
    let mut writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) }, // points to VGA buffer
        // 0xb8000 as a mutable raw pointer. Convert to mutable reference by dereferencing it
        // then immediately borrow again through &mut
    };

    writer.write_byte(b'H');
    writer.write_string("ello ");
    writer.write_string("Wörld!");
}

//==================================================
//==================================================

use volatile::Volatile; 

struct Buffer {
    // Uses Volatile to prevent writing "normally". Must use 'write' method
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                //...

                // Instead of useing '=', uses write method.
                // prevents compiler from optimizing this write
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                //...
            }
        }
    }
    //...
}

//=====================================================
//=====================================================

use core::fmt;

// Formatting macros
impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

// Prints Hello! and the two numbers using "write!"
pub fn print_something() {
    use core::fmt::Write;
    let mut writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    };

    writer.write_byte(b'H');
    writer.write_string("ello! ");
    write!(writer, "The numbers are {} and {}", 42, 1.0/3.0).unwrap();
}

// Move every character one up and start at beginning of last line again
impl Writer {
    fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    // Clearing row by overwriting all of its characters with a space character
    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }
}

//================================================
//================================================

use lazy_static::lazy_static;
use spin::Mutex;

// static WRITER
// Global writer that can be used as interface from other modules without carrying Writer instance around

// statics are initialized at compile time. Normal variables are initialized at run time
// Rust's const evaluator cannot convert raw pointers to references at compile time
// lazy static initializes itself when accessed for first time. makes it initialize at runtime.

// WRITER is immutable!
// To get synchronized interior mutability, use Mutex. Blocks threads when resource is already locked
lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    use core::fmt::Write;
    vga_buffer::WRITER.lock().write_str("Hello again").unwrap();
    write!(vga_buffer::WRITER.lock(), ", some numbers: {} {}", 42, 1.337).unwrap();

    loop {}
}

//================================================
//================================================

// $crate: ensures that macro works from outside std crate.
// format_args macro builds fmt::Arguments type from passed arguments
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
}