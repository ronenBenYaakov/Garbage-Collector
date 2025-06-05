#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use panic_halt as _;
use linked_list_allocator::LockedHeap;

use alloc::boxed::Box;
use core::ptr::NonNull;
use core::ops::Deref;


use embedded::gc::{Heap, Gc, MyData};

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

const HEAP_SIZE: usize = 1024 * 4;
static mut HEAP_MEMORY: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[entry]
fn main() -> ! {
    unsafe {
        ALLOCATOR.lock().init(HEAP_MEMORY.as_ptr() as *mut u8, HEAP_SIZE);
    }

    let mut heap = Heap::new();

    // Allocate some GC objects
    let root1 = heap.allocate(MyData {
        value: 100,
        child: None,
    });

    let root2 = heap.allocate(MyData {
        value: 200,
        child: Some(root1),
    });

    // Print values before GC
    unsafe {
        let data1 = root1.as_any().downcast_ref::<MyData>().unwrap();
        hprintln!("root1 value = {}", data1.value);

        let data2 = root2.as_any().downcast_ref::<MyData>().unwrap();
        hprintln!("root2 value = {}", data2.value);

        if let Some(child) = &data2.child {
            let child_data = child.deref().as_any().downcast_ref::<MyData>().unwrap();
            hprintln!("root2 child value = {}", child_data.value);
        }
    }

    // Collect garbage with both roots alive
    heap.collect_garbage(&[root1.as_non_null(), root2.as_non_null()]);

    // Simulate dropping root2 by collecting garbage with only root1 as root
    heap.collect_garbage(&[root1.as_non_null()]);

    // Print values after GC
    unsafe {
        let data1 = root1.deref().as_any().downcast_ref::<MyData>().unwrap();
        hprintln!("After GC, root1 value = {}", data1.value);
    }

    loop {}
}
