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

    // Perform the actual garbage collection. We use the name `reclaim` to
    // disambiguate from Rust's notion of `collect` on iterators.
    pub(crate) fn reclaim(&self) {
        unimplemented!()
    }

    pub(crate) fn alloc_obj<T: Scan>(&self, object: T) -> Result<*mut T, GcErr> {
        unimplemented!()
    }
}
