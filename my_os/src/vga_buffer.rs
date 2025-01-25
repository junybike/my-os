use volatile::Volatile;
use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;

#[allow(dead_code)]                             // disable warning for unused variant
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color 
{
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);   // Contains full color byte (foreground, background)

impl ColorCode
{
    fn new(foreground: Color, background: Color) -> ColorCode
    {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]  
struct ScreenChar
{
    ascii_character: u8,
    color_code: ColorCode,
}
// repr(C) gurantees that struct's fields are laid out exactly like in a C struct
// gurantees correct field ordering 

#[repr(transparent)]
struct Buffer   // Volatile ensures that we cant accidently write to it "normally"
{
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

pub struct Writer   // Write to the last line and shifts lines up when a line is full
{
    column_position: usize,         // keep track of current position in last row
    color_code: ColorCode,          // foreground and background color specification
    buffer: &'static mut Buffer,    // reference to the VGA buffer
}

impl Writer
{
    pub fn write_byte(&mut self, byte: u8)
    {
        match byte
        {
            b'\n' => self.new_line(),   // If the byte is a newline byte, it doesnt print anything
                                        // calls new_line() instead
            byte=> 
            {
                if self.column_position >= BUFFER_WIDTH // current line is full
                {
                    self.new_line();    
                }
                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;

                // By using write method instead of assignment '=',
                // it gurantees that compiler will never optimize away this write
                self.buffer.chars[row][col].write(ScreenChar
                {
                    ascii_character: byte,
                    color_code,
                });
                self.column_position += 1;
            }
        }
    }

    // If newlines and characters don't fit into the line anymore,
    // move every character one line up and start at beginning of last line again
    // Iterate over all screen characters and move each character one row up
    fn new_line(&mut self)  
    {
        for row in 1..BUFFER_HEIGHT
        {
            for col in 0..BUFFER_WIDTH
            {
                let character = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }
    // Clears a row by overwritting all its characters with a space character
    fn clear_row(&mut self, row: usize)
    {
        let blank = ScreenChar
        {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH
        {
            self.buffer.chars[row][col].write(blank);
        }
    }

    pub fn write_string(&mut self, s: &str)   // convert to bytes and print one by one
    {
        for byte in s.bytes()
        {
            match byte  // to differentiate printable ASCII bytes
            {
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                _ => self.write_byte(0xfe), // prints a white box if its unprintable
            }
        }
    }
}

impl fmt::Write for Writer
{
    fn write_str(&mut self, s: &str) -> fmt::Result
    {
        self.write_string(s);
        Ok(())
    }
}

// Const evaluator: statics are initialized at compile time. (normal variables initialized at run time)
// Rust cannot covert raw pointers to reference at compile time.
// lazy_static initializes itself when accessed for the first time
lazy_static!
{
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer
    {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe {&mut *(0xb8000 as *mut Buffer)},
    });
}

#[macro_export]
macro_rules! print
{
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println 
{
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

// Locks the static WRITER and calls write_fmt method on it
// Must be public since macros need to call _print outside of module but its a private implemenation detail
// doc(hidden) hides the implementation detail from generated documentation
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) 
{
    use core::fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
}