use linked_list_allocator::LockedHeap;
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

pub struct Dummy;

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024;    // 100 KiB
// trying to use this heap region will result in page fault since virtual memory region is not mapped to physical memory yet
// must have init_heap that maps the heap page

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();
// LockedHeap uses Spinklock type for synchronization
// to allow multiple threads access ALLOCATOR static at the same time
// Do not perform allocations in interrupt handlers (may run at arbitrary time and interrupt an in-progress allocation)
//---
// Must initialize allocator after creating the heap
// since it uses empty constructor function which creates an allocator without any backing memory

// the struct has no fields. creates it as zero-sized type
unsafe impl GlobalAlloc for Dummy
{
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8
    {
        null_mut()
    }
    // since allocator never returns any memory, call to dealloc should never occur
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout)
    {
        panic!("dealloc should be never called")
    }
}

// takes mutable reference to a Mapper and FrameAllocator instance.
// both are limited to 4KiB pages by using Size4KiB as generic parameter
// return value: Reuslt with type () as success and MapToError as error variant (error type returned by Mapper::map_to)
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> 
{
    // To create a range of pages that we want to map,
    // convert HEAP_START pointer to VirtAddr then calculate heap end address from it by adding the HEAP_SIZE
    // To get an inclusive bound (address of last byte of heap), subtracts 1.
    // Then convert address to Page types using containing_address function
    // Creates page range from start and end pages using range_inclusive function 
    let page_range = 
    {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    // To map all pages of page range we created
    // For each page,
    // allocate physical frame that page should be mapped to using allocate_frame method. returns None when no more frames left
    // deals with that case by mapping to FrameAllocationFailed error through ok_or method
    // then apply ? to return early in case of error
    //---
    // must set required PRESENT flag and WRITABLE flag for page
    // with these flags, both read and write accesses are allowed 
    // ---
    // uses map_to method to create mapping in active page table.
    // uses ? in case the method fails and forward the error to caller
    // method returns MapperFlush instance to use for updating translation lookaside buffer with flush method
    for page in page_range 
    {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush()
        };
    }

    // locks the inner spinlock of LockedHeap to get exclusive reference to wrapped Heap instance
    // Then calls init method with heap bounds as arguments
    // Since init function already tries to write to heap memory, must initialize heap only after mapping the heap pages
    // After initializing the heap, can use all allocation and collection types of built in alloc crate without error
    unsafe 
    {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}

