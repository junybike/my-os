use x86_64::{structures::paging::PageTable, VirtAddr, PhysAddr};
use x86_64::structures::paging::OffsetPageTable;

use x86_64::structures::paging::{Page, PhysFrame, Mapper, Size4KiB, FrameAllocator};
use bootloader::bootinfo::MemoryMap;
use bootloader::bootinfo::MemoryRegionType;

// Frame allocator returns usable frames from bootloader's memory map
pub struct BootInfoFrameAllocator
{
    // memory map consists of a list of MemoryRegion structs
    // contains start address, length, and type of each memory region
    memory_map: &'static MemoryMap, // static reference to memory map passed by bootloader
    next: usize,                    // keep track of number of next frame that allocator should return
}
impl BootInfoFrameAllocator
{
    // Creates FrameAllocator from passed memory map
    // Unsafe
    // Must gurantee that passed memory map is valid.
    // Requirement: All frames that are marked as USABLE in it are really unused
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self
    {
        BootInfoFrameAllocator
        {
            memory_map,
            next: 0,
        }
    }
    // initializes a BootInfoFrameAllocator with a given memory map
    // next field increases for every frame allocation to avoid  returning same frame twice
    // since we dont know if usable frames of memory map were already used, init must be unsafe to require additional gurantees from caller
}

// returning mutable reference to active level 4 table
// unsafe 
// caller must gurantee that complete physical memory is mapped to virtual memory at passed 'physical_memory_offset'
// must be called only once to avoid aliasing &mut references 
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable
{
    use x86_64::registers::control::Cr3;    // read physical frame of active level 4 table from CR3 register

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();     // takes physical start address
    let virt = physical_memory_offset + phys.as_u64();  // convert to u64 and add to physical_memory_offset 
                                                                  // to get virtual address where the page table frame is mapped
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();       // convert virtual address to *mut PageTable raw pointer through as_mut_ptr method
                                                                  // unsafely creates &mut PageTable reference from it

    &mut *page_table_ptr // unsafe
}

// to translate virtual to physical address, traverse the four level page table until reaching mapped frame
// Translates given virtual address to the mapped physical address or None if address is not mapped
// Unsafe
// caller must gurantee that the complete physical memory is mapped to virtual memory at passed 'physical_memory_offset' 
pub unsafe fn translate_addr(addr: VirtAddr, physical_memory_offset: VirtAddr) -> Option<PhysAddr>
{
    translate_addr_inner(addr, physical_memory_offset)
}

// safe to limit the scope of 'unsafe' because Rust treats whole body of unsafe functions as unsafe block
// must be reachable through unsafe fn from outside of this module
fn translate_addr_inner(addr: VirtAddr, physical_memory_offset: VirtAddr) -> Option<PhysAddr>
{
    // VirtAddr provides methods to compute indexes into page tables of the four levels
    // Stores these indexes in a small array. Allowws us to traverse the page tables using a for loop
    use x86_64::structures::paging::page_table::FrameError;
    use x86_64::registers::control::Cr3;

    // read the active level 4 frame from the CR3 register
    let (level_4_table_frame, _) = Cr3::read();

    let table_indexes = [
        addr.p4_index(), addr.p3_index(), addr.p2_index(), addr.p1_index()
    ];
    let mut frame = level_4_table_frame;

    // traverse the multi-level page table
    for &index in &table_indexes 
    {
        // convert the frame into a page table reference
        let virt = physical_memory_offset + frame.start_address().as_u64();
        let table_ptr: *const PageTable = virt.as_ptr();
        let table = unsafe {&*table_ptr};

        // to retrieve the mapped frame
        // read the page table entry and update `frame`
        let entry = &table[index];
        frame = match entry.frame() {
            Ok(frame) => frame,
            Err(FrameError::FrameNotPresent) => return None,
            Err(FrameError::HugeFrame) => panic!("huge pages not supported"),
        };
    }
    // last visited frame to calculate physical address
    // the frame points to page table frames while iterating and to mapped frame after the last iteration
    // calculate the physical address by adding the page offset
    Some(frame.start_address() + u64::from(addr.page_offset()))
}

// Initializes new OffsetPageTable
// Unsafe
// caller must gurantee the complete physical memory is mapped to virtual memory at the passed 'physical_memory_offset'
// must only called once to avoid aliasing &mut references
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> 
{
    let level_4_table = active_level_4_table(physical_memory_offset);   // to retrieve mutable ref to level 4 page table
    OffsetPageTable::new(level_4_table, physical_memory_offset) // new function expects virtual address at which the mapping of physical memory starts
}
// takes physical_memory_offset as arg and returns new OffsetPageTable instance with a 'static lifetime
// instance stays valid for complete runtime of kernel

// expects a mutable reference to OffsetPageTable instance and frame_allocator
// frame_allocator uses imple Trait syntax to be generic over all types that implement FrameAllocator trait
pub fn create_example_mapping(page: Page, mapper: &mut OffsetPageTable, frame_allocator: &mut impl FrameAllocator<Size4KiB>)
{
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    let flags = Flags::PRESENT | Flags::WRITABLE;

    // map_to is unsafe. Caller must ensure that frame is not already in use.
    // Mapping same frame twice will result in undefined behavior
    let map_to_result = unsafe 
    {
        // FIXME: this is not safe, we do it only for testing
        mapper.map_to(page, frame, flags, frame_allocator)
    };
    map_to_result.expect("map_to failed").flush();
}

pub struct EmptyFrameAllocator;

// Unsafe
// Must gurantee that allocator yields only unused frames.
// Possible undefined behavior: two virtual pages mapped to same physical frame
unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}



impl BootInfoFrameAllocator 
{
    /// Returns an iterator over the usable frames specified in the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> 
    {
        // get usable regions from memory map
        let regions = self.memory_map.iter();
        let usable_regions = regions
            .filter(|r| r.region_type == MemoryRegionType::Usable);
        // map each region to its address range
        let addr_ranges = usable_regions
            .map(|r| r.range.start_addr()..r.range.end_addr());
        // transform to an iterator of frame start addresses
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // create `PhysFrame` types from the start addresses
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}
// Iterator combinator method to transform initial MemoryMap into iterator of usable physical frames
// 1. call iter to convert memory map to an iterator of MemoryRegion
// 2. use filter to skip reserved or otherwise unavailable regions (bootloader updates memory map for all mapping it creates)
// 3. use map combinator and Rust's range syntax to transform iterator of memory regions iterator of address ranges
// 4. use flat_map to transform address ranges into an iterator of frame start addresses, choosing every 4096th address using step_by
//      4096 is the page size. Gets start address of each frame
// 5. convert start address to PhysFrame to construct Iterator<Item = PhysFrame>

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator 
{
    fn allocate_frame(&mut self) -> Option<PhysFrame> 
    {
        // usable_frames: to get an iterator of usable frames from memory map.
        // nth: gets frame with index self.next
        // before returning, increase self.next so that we return the following frame on next call
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}