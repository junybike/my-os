unsafe impl GlobalAlloc for BumpAllocator
{
    unsafe fn alloc(&self, layout: Layout) -> *mut u8
    {
        // since alloc/dealloc methods of GlobalAlloc only operate on immutable &self reference
        // updating next and allocations field is not possible
        // Reason
        // global heap allocator is defined by adding #[global_allocator] to a static that implements GlobalAlloc trait
        // static variables are immutable in rust. so cant call method that takes &mut self on static allocator
        // so all methods of GlobalAlloc only takes immutable self reference
        // Solution
        // use spin::Mutex wrapper type to implement GlobalAlloc trait for bump allocator
        // implement trait for spin::Mutex<BumpAllocator>
        let alloc_start = self.next;
        self.next = alloc_start + layout.size();
        self.allocations += 1;
        alloc_start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        todo!();
    }
}

//---------------

/// Align the given address `addr` upwards to alignment `align`.
fn align_up_old(addr: usize, align: usize) -> usize 
{
    let remainder = addr % align;
    if remainder == 0 
    {
        addr // addr already aligned
    } else 
    {
        addr - remainder + align
    }
}

//------------------------

struct ListNode 
{
    size: usize,
    next: Option<&'static mut ListNode>,    // an owned object behind a pointer. 
                                            // Box without a destructor that frees object at the end of the scope
}

pub struct LinkedListAllocator 
{
    head: ListNode, // points to the first heap region
}

impl LinkedListAllocator 
{
    /// Creates an empty LinkedListAllocator.
    pub const fn new() -> Self 
    {
        Self 
        {
            head: ListNode::new(0),
        }
    }

    /// Initialize the allocator with the given heap bounds.
    ///
    /// This function is unsafe because the caller must guarantee that the given
    /// heap bounds are valid and that the heap is unused. This method must be
    /// called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) 
    {
        self.add_free_region(heap_start, heap_size);
    }

    /// Adds the given memory region to the front of the list.
    /// fundamental push operation
    /// takes address and size of memory region as arg and adds to the front of the list
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) 
    {
        // ensure that the freed region is capable of holding ListNode
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        assert!(size >= mem::size_of::<ListNode>());

        // create a new list node and append it at the start of the list
        let mut node = ListNode::new(size);
        node.next = self.head.next.take();
        let node_ptr = addr as *mut ListNode;
        node_ptr.write(node);
        self.head.next = Some(&mut *node_ptr)
    }

    /// Looks for a free region with the given size and alignment and removes
    /// it from the list.
    ///
    /// Returns a tuple of the list node and the start address of the allocation.
    fn find_region(&mut self, size: usize, align: usize) -> Option<(&'static mut ListNode, usize)>
    {
        // reference to current list node, updated for each iteration
        let mut current = &mut self.head;
        // look for a large enough memory region in linked list
        while let Some(ref mut region) = current.next 
        {
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) 
            {
                // region suitable for allocation -> remove node from list
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } 
            else 
            {
                // region not suitable -> continue with next region
                current = current.next.as_mut().unwrap();
            }
        }

        // no suitable region found
        None
    }

    /// Try to use the given region for an allocation with given size and
    /// alignment.
    ///
    /// Returns the allocation start address on success.
    fn alloc_from_region(region: &ListNode, size: usize, align: usize) -> Result<usize, ()>
    {
        // calculating start and end address of a potential allocation
        let alloc_start = align_up(region.start_addr(), align);         
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        // check in case of allocation not fitting a suitable region perfectly
        if alloc_end > region.end_addr() 
        {
            // region too small
            return Err(());
        }

        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() 
        {
            // rest of region too small to hold a ListNode (required because the
            // allocation splits the region in a used and a free part)
            return Err(());
        }

        // region suitable for allocation
        Ok(alloc_start)
    }  

    /// Adjust the given layout so that the resulting allocated memory
    /// region is also capable of storing a `ListNode`.
    ///
    /// Returns the adjusted size and alignment as a (size, align) tuple.
    fn size_align(layout: Layout) -> (usize, usize) 
    {
        // align_to passes Layout to increase the alignment to the alignment of a ListNode if necessary
        // then uses pad_to_align to round up size of multiple alignment to ensure that start address of next memory block have the correct alignment for storing ListNode
        // uses max method to enforce minimum allocation size of mem::size_of::<ListNode>
        let layout = layout
            .align_to(mem::align_of::<ListNode>())
            .expect("adjusting alignment failed")
            .pad_to_align();
        let size = layout.size().max(mem::size_of::<ListNode>());
        (size, layout.align())
    }
}

// For a wrapped Locked<LinkedListAllocator>
// The locked wrapper adds interior mutability through a spinlock
// which allow allocator instance modification despite alloc and dealloc methods only take &self references 
unsafe impl GlobalAlloc for Locked<LinkedListAllocator> 
{
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 
    {
        // perform layout adjustments and calls lock to receive mutable allocator reference
        let (size, align) = LinkedListAllocator::size_align(layout);
        let mut allocator = self.lock();

        // finding a suitable memory region for allocation and remove it from list
        // returns null_mut to signal an error if theres no suitable memory region
        // in success, returns tuple of suitable region and start address of the allocation
        if let Some((region, alloc_start)) = allocator.find_region(size, align) 
        {
            // calculates the end address of allocation and excess size again
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;

            // if excess size is not null, calls add_free_region to add excess size of memory region back to free list
            if excess_size > 0 
            {
                allocator.add_free_region(alloc_end, excess_size);
            }

            // returns alloc_start address casted as a *mut u8
            alloc_start as *mut u8
        } 
        else 
        {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) 
    {
        // perform layout adjustments and retrieves LinkedListAllocator reference by calling Mutex::lock on Locked wrapper 
        let (size, _) = LinkedListAllocator::size_align(layout);

        // add deallocated region to the free list
        self.lock().add_free_region(ptr as usize, size)
    }
}

// bump 
// Linked List
// Fixed size block