pub unsafe trait GlobalAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8;
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout);

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 { ... }
    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize
    ) -> *mut u8 { ... }
}

/*
alloc: takes layout instance as arg which describes desired sizee and alignment that allocated memory should have
- returns raw pointer to first byte of allocated memory block
- returns null pointer to signal an allocation error
dealloc: free a memory block
- receives two args: pointer returned by alloc and layout that was used for allocation

alloc_zeroed: method to calling alloc and set allocated memory block to zero
- the default implementation
- an allocator implementation can override default implementation with more efficient custom implementation
realloc: method to grow or shrink an allocation
- Default implementation allocates new memory block with desired size and copies all content from previous allocation
- allocator implementation can provide more efficient implementation of this method 

trait it self and all trait method must be declared as unsafe
- programmer must gurantee that trait implementation for allocator type is correct
    alloc must never return a memory block that is already used. it will cause undefined behavior
- caller must ensure various invariants when calling methods.

*/

//--------------------

#[global_allocator]
static ALLOCATOR: Dummy = Dummy;

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

//----------------------

fn kernel_main(boot_info: &'static BootInfo) -> ! 
{
    // […] print "Hello World!", call `init`, create `mapper` and `frame_allocator`

    // panics because Box::new implicitly calls alloc function of global allocator.
    // the dummy allocator always returns null pointer
    let x = Box::new(41);

    // […] call `test_main` in test mode

    println!("It did not crash!");
    blog_os::hlt_loop();
}

//--------------------

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024;    // 100 KiB
// trying to use this heap region will result in page fault since virtual memory region is not mapped to physical memory yet
// must have init_heap that maps the heap page

//----------------------

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

//-------------------------

fn kernel_main(boot_info: &'static BootInfo) -> !
{
    use my_os::allocator;
    use my_os::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    println!("Hellow world{}", "!");
    my_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };

    // new
    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    // allocate a number on the heap
    let heap_value = Box::new(41);
    println!("heap_value at {:p}", heap_value);

    // create a dynamically sized vector
    let mut vec = Vec::new();
    for i in 0..500 
    {
        vec.push(i);
    }
    // prints underlying heap pointers using {:p} formatting specifier
    println!("vec at {:p}", vec.as_slice());

    // create a reference counted vector -> will be freed when count reaches 0
    let reference_counted = Rc::new(vec![1, 2, 3]);
    let cloned_reference = reference_counted.clone();
    println!("current reference count is {}", Rc::strong_count(&cloned_reference));
    core::mem::drop(reference_counted);
    println!("reference count is {} now", Rc::strong_count(&cloned_reference));

    #[cfg(test)]
    test_main();
    
    println!("Did not crash :o");
    my_os::hlt_loop();
}