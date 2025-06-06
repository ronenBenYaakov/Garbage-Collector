#![no_std]
extern crate alloc;

use core::cell::{Cell, RefCell};
use core::ptr::NonNull;
use core::any::Any;
use core::ops::Deref;
use alloc::{boxed::Box, vec::Vec};
use cortex_m_semihosting::hprintln;

/// Trait for GC-traceable objects
pub trait Trace {
    fn trace(&self);
    fn as_any(&self) -> &dyn Any;
}

/// GC-managed data structure
pub struct MyData {
    pub value: i32,
    pub child: Option<Gc<dyn Trace>>,
}

impl Trace for MyData {
    fn trace(&self) {
        if let Some(child) = &self.child {
            child.trace();
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Drop for MyData {
    fn drop(&mut self) {
        hprintln!("Dropping MyData with value = {}", self.value);
    }
}

/// Box that stores traced object and mark bit
pub struct GcBox<T: ?Sized> {
    pub marked: Cell<bool>,
    pub value: Box<T>,
}

impl<T: ?Sized> GcBox<T> {
    pub fn new(value: Box<T>) -> Self {
        GcBox {
            marked: Cell::new(false),
            value,
        }
    }

    fn trace(&self)
    where
        T: Trace,
    {
        self.value.trace();
    }
}

/// GC smart pointer
pub struct Gc<T: ?Sized> {
    ptr: NonNull<GcBox<T>>,
}

impl<T: ?Sized> Copy for Gc<T> {}
impl<T: ?Sized> Clone for Gc<T> {
    fn clone(&self) -> Self {
        Gc { ptr: self.ptr }
    }
}

impl<T: ?Sized> Deref for Gc<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.ptr.as_ref().value }
    }
}

impl<T: Trace + ?Sized> Gc<T> {
    pub unsafe fn from_raw(ptr: NonNull<GcBox<T>>) -> Self {
        Gc { ptr }
    }

    pub fn as_non_null(&self) -> NonNull<GcBox<dyn Trace>> {
        to_dyn_trace_ptr(self.ptr)
    }

    pub fn trace(&self) {
        unsafe {
            let gc_box = self.ptr.as_ref();
            if !gc_box.marked.get() {
                gc_box.marked.set(true);
                gc_box.value.trace();
            }
        }
    }
}

/// Convert GcBox<T> to GcBox<dyn Trace>
pub fn to_dyn_trace_ptr<T: Trace + ?Sized>(ptr: NonNull<GcBox<T>>) -> NonNull<GcBox<dyn Trace>> {
    unsafe { NonNull::new_unchecked(ptr.as_ptr() as *mut GcBox<dyn Trace>) }
}

/// The Heap tracks all allocations and roots
pub struct Heap {
    objects: Vec<NonNull<GcBox<dyn Trace>>>,
    pub roots: RefCell<Vec<NonNull<GcBox<dyn Trace>>>>,
    allocation_count: usize,
    threshold: usize,
}

impl Heap {
    pub fn new() -> Self {
        Heap {
            objects: Vec::new(),
            roots: RefCell::new(Vec::new()),
            allocation_count: 0,
            threshold: 1,
        }
    }

    pub fn allocate<T: Trace + 'static>(&mut self, value: T) -> Gc<dyn Trace> {
        let boxed: Box<dyn Trace> = Box::new(value);
        let gc_box = Box::new(GcBox::new(boxed));
        let ptr = unsafe { NonNull::new_unchecked(Box::into_raw(gc_box)) };
        self.objects.push(ptr);
        self.allocation_count += 1;

        if self.allocation_count >= self.threshold {
            let roots = self.roots.borrow().clone();
            self.collect_garbage(&roots);
            self.allocation_count = 0;
        }

        unsafe { Gc::from_raw(ptr) }
    }

    pub fn register_root(&self, ptr: NonNull<GcBox<dyn Trace>>) {
        let mut roots = self.roots.borrow_mut();
        if !roots.contains(&ptr) {
            roots.push(ptr);
        }
    }

    pub fn unregister_root(&self, ptr: NonNull<GcBox<dyn Trace>>) {
        let mut roots = self.roots.borrow_mut();
        roots.retain(|&r| r != ptr);
    }

    pub fn collect_garbage(&mut self, roots: &[NonNull<GcBox<dyn Trace>>]) {
        for obj in &self.objects {
            unsafe {
                obj.as_ref().marked.set(false);
            }
        }

        for &root in roots {
            unsafe {
                let obj = root.as_ref();
                if !obj.marked.get() {
                    obj.marked.set(true);
                    obj.value.trace();
                }
            }
        }

        self.objects.retain(|&ptr| {
            let keep = unsafe { ptr.as_ref().marked.get() };
            if !keep {
                unsafe {
                    drop(Box::from_raw(ptr.as_ptr()));
                }
            }
            keep
        });
    }
}

/// RAII root registration
pub struct RootGuard<'a> {
    heap: &'a Heap,
    ptr: NonNull<GcBox<dyn Trace>>,
}

impl<'a> RootGuard<'a> {
    pub fn new(heap: &'a Heap, gc: Gc<dyn Trace>) -> Self {
        let ptr = gc.as_non_null();
        heap.register_root(ptr);
        RootGuard { heap, ptr }
    }
}

impl<'a> Drop for RootGuard<'a> {
    fn drop(&mut self) {
        self.heap.unregister_root(self.ptr);
    }
}
