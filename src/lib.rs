//! This file contains the API for the runtime GC. Interaction with the GC is
//! only legal through these functions.
//!
//! The GC runtime consists of a mutable singleton `Collector` struct:
//!
//! ```rust, ignore
//! struct Collector {
//!     // Heap bookkeeping information
//!     hstart: *const usize,
//!     hptr: *mut usize,
//!
//!     // Flag to determine whether to collect at the next safepoint
//!     collect_next: bool,
//!
//!     // The in-memory safepoint table used to identify roots
//!     roots: HashMap<ReturnAddress, SafepointRoots>
//! }

#[cfg(not(all(target_pointer_width = "64", target_arch = "x86_64")))]
compile_error!("Requires x86_64 with 64 bit pointer width.");

mod collector;
mod safepoints;
use collector::Collector;

// FIXME: This will be replaced with the `Scan` trait lang item in our forked
// rustc's libcore. For now, we define `Scan` at the top level in this library.
pub trait Scan {
    fn scan(&self) {}
}

pub enum GcErr {
    OOM(String),
}


thread_local!(static COLLECTOR: Collector =  Collector::new());

/// This must be called before the GC can be used (usually in the setup code
/// before `main()`). Initialisation consists of two stages:
///     1. Read the stackmap section in the ELF file into an in-memory table for
///        fast lookup.
///     2. Allocate a chunk of heap memory to be used to store objects managed
///        by the GC.
pub fn init() {
    unimplemented!();
}

/// This function is the *only* way that a collection can be triggered. Calls to
/// `safepoint_poll` are generated by LLVM's InsertSafepoints opt pass. They are
/// inserted liberally into the mutator's code at all function calls and
/// loop-backedges.
///
/// In almost all cases, a safepoint poll will *never* trigger a collection.
/// It's therefore really important that this function returns fast on a
/// wont-collect poll.
///
/// No information about whether a poll resulted in a collection is returned to
/// the mutator. The only thing that can be guaranteed is that a collection
/// *might* have happened after returning from this call.
///
/// ------------------------------ WARNING -------------------------------------
/// | We need to be really careful about code which panics inside the collector|
/// | here. The safepoint poll will *not* be called by native Rust code which  |
/// | means that panic handling is UB. We should probably abort on panic here. |
/// ----------------------------------------------------------------------------
#[no_mangle]
pub extern "C" fn safepoint_poll() {
    if COLLECTOR.with(|c| c.should_collect()) {
        COLLECTOR.with(|c| c.reclaim())
    }
}

/// Blocks the mutator to perform a collection. As this is a single threaded GC
/// implementation, we can guarantee that this will take place immediately a
/// safepoint will be inserted into the `force_collect` function prologue.
pub fn force_collect() {
    COLLECTOR.with(|c| c.reclaim());
}

/// Attempts to store an object in the GC heap and return a raw pointer on
/// success. `alloc_raw` should not be called directly by the user. Instead, it
/// is exposed so that the standard library can build a GC smart pointer to a
/// managed object. All allocation to the GC heap *must* go through `alloc_raw`.
///
/// It is UB to:
///     - Call `alloc_raw` directly in user code.
///     - Allocate to the GC heap in any way other than through `alloc_raw`.
///     - Allow the returned pointer to live past a safepoint boundary (function
///     call; loop-backedge; new GC allocation) *UNLESS* it has been placed in
///     a container which implements the `Scan` trait, with a `scan()` method
///     to inform the collector that it is, indeed, a valid GC pointer.
pub fn alloc_raw<T: Scan>(object: T) -> Result<*mut T, GcErr> {
    COLLECTOR.with(|c| c.alloc_obj(object))
}
