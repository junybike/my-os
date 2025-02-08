use alloc::alloc::{GlobalAlloc, Layout};
use super::{align_up, Locked};
use core::ptr;
pub struct BumpAllocator
{
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
}
impl BumpAllocator 
{
    /// Creates a new empty bump allocator.
    /// 
    /// heap_start and heap_end keep track of lower and upper bounds of heap memory region
    pub const fn new() -> Self 
    {
        BumpAllocator 
        {
            heap_start: 0,
            heap_end: 0,
            next: 0,            // always point to start address of next allocation
            allocations: 0,     // counter for active allocations with goal of resetting allocator after the last allocation has freed
        }
    }

    /// Initializes the bump allocator with the given heap bounds.
    ///
    /// This method is unsafe because the caller must ensure that the given
    /// memory range is unused. Also, this method must be called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) 
    {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
    }
}

unsafe impl GlobalAlloc for Locked<BumpAllocator>
{
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 
    {
        // get a mutable reference
        // locked until the end of method so that no data race occur in multithreaded contexts
        let mut bump = self.lock(); 

        // rounds up the next address to alignment specified by Layout argument
        // then add reqeusted allocation size to alloc_start to get end address of allocation
        // checked_add: to prevent integer overflow on large allocation
        let alloc_start = align_up(bump.next, layout.align());
        let alloc_end = match alloc_start.checked_add(layout.size()) 
        {
            Some(end) => end,           
            None => return ptr::null_mut(),
        };

        if alloc_end > bump.heap_end // out of memory
        {
            ptr::null_mut() 
        } 
        else    // if no overflow, return next address
        {
            bump.next = alloc_end;
            bump.allocations += 1;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) 
    {
        // get a mutable reference
        let mut bump = self.lock(); 

        // just decrease allocation counter. 
        // if it hits 0, resets next address to heap_start to make complete heap memory available again
        bump.allocations -= 1;
        if bump.allocations == 0 {
            bump.next = bump.heap_start;
        }
    }
}

