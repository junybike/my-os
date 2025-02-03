// use x86_64::registers::segmentation::Segment;
use x86_64::VirtAddr;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor, SegmentSelector};
use lazy_static::lazy_static;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

struct Selectors
{
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

pub fn init()
{   // uses selector to reload the cs register and load TSS
    use x86_64::instructions::tables::load_tss;
    use x86_64::instructions::segmentation::{CS, Segment};

    GDT.0.load();
    unsafe 
    {   // Unsafe: May be possible to break memory safety by loading invalid selectors
        CS::set_reg(GDT.1.code_selector);   // set_reg reloads code segment register
        load_tss(GDT.1.tss_selector);       // load_tss load the TSS
    }
}

lazy_static! 
{
    static ref TSS: TaskStateSegment = 
    {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = 
        {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];   // stack storage

            let stack_start = VirtAddr::from_ptr(&raw const STACK);
            let stack_end = stack_start + STACK_SIZE;
            stack_end
        };
        tss
    };
}

// Provides access to code_selector and tss_selector
lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = 
    {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
        (gdt, Selectors{code_selector, tss_selector})
    };
}