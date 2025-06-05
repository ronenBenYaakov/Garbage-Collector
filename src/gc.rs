#![no_std]
extern crate alloc;

use core::cell::Cell;
use alloc::{boxed::Box, vec::Vec};
use core::ptr::NonNull;
use core::any::Any;
use core::ops::Deref;

/// Trait for objects that can be traced by the GC
pub trait Trace {
    fn trace(&self);
    fn as_any(&self) -> &dyn Any;
}

impl<T: Trace + ?Sized> GcBox<T> {
    fn trace(&self) {
        self.value.trace();
    }
}


/// Example GC-managed data struct with possible child reference
pub struct MyData {
    pub value: i32,
    pub child: Option<Gc<dyn Trace>>,
}

impl Trace for MyData {
    fn trace(&self) {
        if let Some(ref child) = self.child {
            child.trace();
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// GC pointer wrapper around a pointer to GcBox<T>
pub struct Gc<T: ?Sized> {
    ptr: NonNull<GcBox<T>>,
}

impl<T: Trace + ?Sized> Gc<T> {
    /// Unsafe: create Gc from a raw NonNull pointer
    pub unsafe fn from_raw(ptr: NonNull<GcBox<T>>) -> Self {
        Gc { ptr }
    }

    /// Get NonNull pointer to the inner GcBox
    pub fn as_non_null(&self) -> NonNull<GcBox<T>> {
        self.ptr
    }

    /// Trace this object (marking)
    pub fn trace(&self) {
        unsafe {
            let gc_box = self.ptr.as_ref();
            if !gc_box.marked.get() {
                gc_box.marked.set(true);
                // Recursively trace inner references
                gc_box.value.as_ref().trace();
            }
        }
    }
}



// Convert NonNull<GcBox<MyData>> to NonNull<GcBox<dyn Trace>>
pub fn to_dyn_trace_ptr<T: Trace + ?Sized>(ptr: NonNull<GcBox<T>>) -> NonNull<GcBox<dyn Trace>> {
    // SAFETY: This is safe because GcBox<T> and GcBox<dyn Trace> have the same layout
    unsafe { NonNull::new_unchecked(ptr.as_ptr() as *mut GcBox<dyn Trace>) }
}


impl<T: ?Sized> Clone for Gc<T> {
    fn clone(&self) -> Self {
        Gc { ptr: self.ptr }
    }
}

impl<T: ?Sized> Copy for Gc<T> {}

impl<T: ?Sized> Deref for Gc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.ptr.as_ref().value }
    }
}

/// The GC Box holding an allocated object and mark bit
pub struct GcBox<T: ?Sized> {
    pub marked: Cell<bool>,
    pub value: Box<T>, // Box allows unsized types like dyn Trace
}

impl<T: ?Sized> GcBox<T> {
    pub fn new(value: Box<T>) -> Self {
        GcBox {
            marked: Cell::new(false),
            value,
        }
    }
}

/// The Heap manages all GC allocations and roots
pub struct Heap {
    objects: Vec<NonNull<GcBox<dyn Trace>>>, // All allocated objects
    roots: Vec<NonNull<GcBox<dyn Trace>>>,   // Registered roots
}

impl Heap {
    /// Create a new empty heap
    pub fn new() -> Self {
        Heap {
            objects: Vec::new(),
            roots: Vec::new(),
        }
    }

    /// Allocate a new object on the heap, returning a Gc pointer
    pub fn allocate<T: Trace + 'static>(&mut self, value: T) -> Gc<dyn Trace> {
        let trait_obj: Box<dyn Trace> = Box::new(value);
        let gc_box = Box::new(GcBox::new(trait_obj));
        let ptr = unsafe { NonNull::new_unchecked(Box::into_raw(gc_box)) };
        self.objects.push(ptr);
        unsafe { Gc::from_raw(ptr) }
    }

    /// Register a root to keep alive
    pub fn register_root(&mut self, root: NonNull<GcBox<dyn Trace>>) {
        if !self.roots.contains(&root) {
            self.roots.push(root);
        }
    }

    /// Unregister a root (no longer root)
    pub fn unregister_root(&mut self, root: NonNull<GcBox<dyn Trace>>) {
        self.roots.retain(|&r| r != root);
    }

    /// Perform garbage collection using all registered roots
    pub fn collect_garbage(&mut self, roots: &[NonNull<GcBox<dyn Trace>>]) {
    // Unmark all objects
    for obj in &self.objects {
        unsafe { obj.as_ref().marked.set(false); }
    }

    // Mark all reachable objects from roots
    for &root in roots {
        unsafe {
            let gc_obj = root.as_ref();
            if !gc_obj.marked.get() {
                gc_obj.marked.set(true);
                gc_obj.value.trace();
            }
        }
    }

    // Sweep unreachable objects
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

/// RAII guard to automatically register/unregister roots on the heap
pub struct RootGuard<'a> {
    heap: &'a mut Heap,
    ptr: NonNull<GcBox<dyn Trace>>,
}

impl<'a> RootGuard<'a> {
    /// Create a new root guard registering the root on the heap
    pub fn new(heap: &'a mut Heap, gc: Gc<dyn Trace>) -> Self {
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
