use alloc::alloc::Layout;
use core::ptr;
use super::Locked;
use alloc::alloc::GlobalAlloc;
use core::{mem, ptr::NonNull};

struct ListNode
{
    // no size field since every block has same size with fixed size block allocator design
    next: Option<&'static mut ListNode>,
}

/// The block sizes to use.
///
/// The sizes must each be power of 2 because they are also used as
/// the block alignment (alignments must be always powers of 2).
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

pub struct FixedSizeBlockAllocator 
{
    // list head is an array of head pointers, one for each block size
    // fallback allocator: for allocation larger than largest block size 
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    fallback_allocator: linked_list_allocator::Heap,
}
impl FixedSizeBlockAllocator 
{
    /// Creates an empty FixedSizeBlockAllocator.
    /// Initializes list_heads with empty node and creates empty linked list allocator as fallback_allocator
    pub const fn new() -> Self 
    {
        const EMPTY: Option<&'static mut ListNode> = None;
        FixedSizeBlockAllocator 
        {
            // EMPTY is needed to tell Rust compiler that we're initializing array with constant value
            // None; BLOCK_SIZE.len() does not work since compiler requires Option<&'static mut ListNode> to implement copy trait
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    /// Initialize the allocator with the given heap bounds.
    ///
    /// This function is unsafe because the caller must guarantee that the given
    /// heap bounds are valid and that the heap is unused. This method must be
    /// called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) 
    {
        self.fallback_allocator.init(heap_start, heap_size);
    }

    /// Allocates using the fallback allocator.
    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 
    {
        match self.fallback_allocator.allocate_first_fit(layout) 
        {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => ptr::null_mut(),
        }
    }
    // linked_list_allocator crate do not implement GlobalAlloc (not possible without locking)
    // it provides allocate_first_fit. It returns Result<NonNull<u8>, ()>
    // NonNull: abstraction for raw pointer that is guranteed to not be a null pointer
    // By mapping Ok case to NonNull::as_ptr and Err case to a null pointer, we can translate this to *mut u8 type
}

/// Returns lowest possible block size for a given Layout
fn list_index(layout: &Layout) -> Option<usize> 
{
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}
// block must have at least the size and alignment required by given Layout.
// Since block size is an alignment, required_block_size is maximmum of layout's size() and align() attributes
// to find next larger block in BLOCK_SIZE, uses iter() to get iterator and position() to find index of first block that is at least as large as required_block_Size
// returns BLOCK_SIZES slice to use returned index as index into list_heads array

// implements GlobalAlloc directly to allocator with Locked wrapper to add synchronized interior mutability
unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> 
{
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 
    {
        // get mutable reference to wrapped allocator instance with lock
        let mut allocator = self.lock();
        
        // calculate appropriate block size for given Layout and get corresponding index to list_heads array
        match list_index(&layout) 
        {
            Some(index) => 
            {
                match allocator.list_heads[index].take() 
                {
                    // If Some, try to remove first node in corresponding list started by list_heads[index] using Option::take method
                    // if list is not empty we enter the Some(node) branch of match statement (where we point the head pointer of list to successor of popped node)
                    // then returns popped node pointer as *mut u8 
                    Some(node) => 
                    {
                        allocator.list_heads[index] = node.next.take();
                        node as *mut ListNode as *mut u8
                    }
                    // list of blocks is empty -> need to construct new block
                    // gets current block size from BLOCK_SIZE slice and use it as size and alignment for new block
                    // creates new Layout from it and call fallback_alloc to perform allocation
                    // must adjust layout and alignment since block will be added to block list on deallocation
                    None => 
                    {
                        // no block exists in list => allocate new block
                        let block_size = BLOCK_SIZES[index];
                        // only works if all block sizes are a power of 2
                        let block_align = block_size;
                        let layout = Layout::from_size_align(block_size, block_align)
                            .unwrap();
                        allocator.fallback_alloc(layout)
                    }
                }
            }
            None => allocator.fallback_alloc(layout),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) 
    {
        // gets mutable allocator reference and list_index to get block list corresponding to given Layout
        let mut allocator = self.lock();
        match list_index(&layout) 
        {
            // if list_index returns block index, need to add freed memory to list
            // creates new ListNode pointing to current list head
            // check if current block size specified by index has required size and alignent for storing ListNode
            // then perfom write by converting the given *mut u8 pointer to *mut ListNode and call unsafe write method on it
            // Lastly, set head pointer of list (currently None since take is called) to newly written ListNode. Converts raw new_node_ptr to mutable reference
            Some(index) => 
            {
                let new_node = ListNode 
                {
                    next: allocator.list_heads[index].take(),
                };
                // verify that block has size and alignment required for storing node
                assert!(mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);
                assert!(mem::align_of::<ListNode>() <= BLOCK_SIZES[index]);
                let new_node_ptr = ptr as *mut ListNode;
                new_node_ptr.write(new_node);
                allocator.list_heads[index] = Some(&mut *new_node_ptr);
            }
            // if index is None, no fitting block size exists in BLOCK_SIZE
            // allocation was created by fallback allocator
            // must use deallocate to free the memory again
            None => 
            {
                // expects NonNull instead of *mut u8
                // unwrap fails if pointer is null
                let ptr = NonNull::new(ptr).unwrap();
                allocator.fallback_allocator.deallocate(ptr, layout);
            }
        }
    }
}