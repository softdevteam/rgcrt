use std::{
    alloc::{alloc, Layout},
    cell::{Cell, UnsafeCell},
    collections::HashMap,
    path::Path
};

use crate::{
    safepoints::{ReturnAddress, SafepointRoots},
    Scan, GcErr
};

/// The size of the heap in bytes
const HSIZE: usize = 1024;

pub(crate) struct Collector {
    hptr: Cell<*mut usize>,
    hstart: Cell<usize>,
    hend: Cell<usize>,

    collect_next: Cell<bool>,

    roots: UnsafeCell<Option<HashMap<ReturnAddress, SafepointRoots>>>
}

impl Collector {
    pub(crate) fn new() -> Self {
        Collector {
            hptr: Cell::new(0 as *mut usize),
            hstart: Cell::new(0),
            hend: Cell::new(0),

            collect_next: Cell::new(false),
            roots: UnsafeCell::new(None)
        }
    }

    #[inline]
    pub fn collect_next(&self) {
        self.collect_next.set(true);
    }

    #[inline]
    pub fn should_collect(&self) -> bool {
        self.collect_next.get()
    }

    pub fn mk_heap(&self, size: usize) {
        let layout = Layout::array::<u8>(size).unwrap();
        let ptr = unsafe { alloc(layout) as *mut usize };

        if ptr.is_null() {
            panic!("Can't allocate memory.");
        }

        self.hptr.set(ptr);
        self.hstart.set(ptr as usize);
        self.hend.set(ptr as usize + size);
    }

    pub fn mk_root_table<P: AsRef<Path>>(&self, path: P) {
        unimplemented!()
    }

    /// Perform the actual garbage collection. We use the name `reclaim` instead
    /// of collect to disambiguate from Rust's notion of `collect` on iterators.
    pub(crate) fn reclaim(&self) {
        eprintln!("Collection is no-op: not yet implemented")
    }

    /// Reserves a block in memory of the given size, returning a pointer which
    /// can be used by the allocator to copy memory.
    /// XXX: Since `reclaim` is unimplemented, this is just pointer bump until
    /// the heap is OOM.
    fn reserve_block<T>(&self, size: usize) -> Result<*mut T, GcErr> {
        let hptr = self.hptr.get();
        let obj_end = hptr as usize + size;

        if (obj_end as usize) < self.hend.get() {
            self.hptr.set(obj_end as *mut usize);
            Ok(hptr as *mut T)
        } else {
            Err(GcErr::OOM("No free space available".to_string()))
        }
    }

    pub(crate) fn alloc_obj<T: Scan>(&self, object: T) -> Result<*mut T, GcErr> {
        let obj_size = std::mem::size_of::<T>();
        // Try and get a pointer into the heap to store the object, if that
        // fails, we'll perform a GC and try again.
        let hptr = self.reserve_block(obj_size).or_else(|_| {
            self.reclaim();
            self.reserve_block(obj_size)
        })?;

        // Use memcpy to copy `object` to the GC heap because we can guarantee
        // that `object`'s src address will never overlap with its new position
        // on the heap. This is less expensive than `memmove`, as we don't need
        // first move `object` to a temporary buffer.
        unsafe { std::ptr::copy_nonoverlapping::<T>(&object, hptr, 1) };
        Ok(hptr as *mut T)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq, Clone)]
    struct S(usize, u32);
    impl Scan for S {}

    #[test]
    fn simple_alloc() {
        let s = S(1234, 5678);
        let gc = Collector::new();
        gc.mk_heap(1024);

        let raw_gcptr = gc.alloc_obj(s).unwrap();

        let gcval = unsafe { &*raw_gcptr };
        assert_eq!(*gcval, S(1234, 5678));
    }

    #[test]
    fn alloc_err_if_oom() {
        let s = S(1234, 5678);
        let gc = Collector::new();
        gc.mk_heap(32);

        let obj1 = gc.alloc_obj(s.clone());
        let obj2 = gc.alloc_obj(s.clone());
        eprintln!("{:?}", obj2);

        assert!(obj1.is_ok());
        assert!(obj2.is_err());
    }
}
