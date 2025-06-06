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

use embedded::gc::{Gc, Heap, MyData, RootGuard};

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

const HEAP_SIZE: usize = 1024 * 4;
static mut HEAP_MEMORY: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[entry]
fn main() -> ! {
    unsafe {
        ALLOCATOR.lock().init(HEAP_MEMORY.as_ptr() as *mut u8, HEAP_SIZE);
    }

    run_example();

    loop {}
}

/// Demonstrates tracing GC behavior with RAII root registration
use core::cell::RefCell;

fn run_example() {
    let heap = RefCell::new(Heap::new());

    // Allocate root1 and keep it rooted
    {
        let mut heap_ref = heap.borrow_mut();
        let root1 = heap_ref.allocate(MyData {
            value: 100,
            child: None,
        });
        let _guard1 = RootGuard::new(&mut *heap_ref, root1);

        {
            let mut heap_ref = heap.borrow_mut();
            let root2 = heap_ref.allocate(MyData {
                value: 200,
                child: Some(root1),
            });
            let _guard2 = RootGuard::new(&mut *heap_ref, root2);

            unsafe {
                let data1 = root1.deref().as_any().downcast_ref::<MyData>().unwrap();
                let data2 = root2.deref().as_any().downcast_ref::<MyData>().unwrap();

                hprintln!("root1 value = {}", data1.value);
                hprintln!("root2 value = {}", data2.value);

                if let Some(child) = &data2.child {
                    let child_data = child.deref().as_any().downcast_ref::<MyData>().unwrap();
                    hprintln!("root2 child value = {}", child_data.value);
                }
            }

            // Collect GC with both roots alive
            hprintln!("Collecting GC with root1 and root2");
            let roots = heap.borrow().roots.borrow().clone();
            heap.borrow_mut().collect_garbage(&roots);
            // _guard2 drops here
        }

        // Now only root1 is rooted
        hprintln!("Collecting GC with only root1");
        let roots = heap.borrow().roots.borrow().clone();
        heap.borrow_mut().collect_garbage(&roots);

        unsafe {
            let data1 = root1.deref().as_any().downcast_ref::<MyData>().unwrap();
            hprintln!("After GC, root1 value = {}", data1.value);
        }
    }
}
