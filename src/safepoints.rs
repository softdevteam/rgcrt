use std::{collections::HashMap, path::Path};
use ykstackmaps::{LocKind, LocOffset, SMRec, StackMapParser};

use core::mem;

static NUM_SKIP_STACKMAPS: usize = 2;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct ReturnAddress(pub u64);

/// The offset from the stack pointer.
#[derive(Debug)]
struct SPO(u32);

/// A `PtrSlot` identifies a stack root at a given safepoint using its offset
/// from the Stack Pointer.
///
/// A base pointer (not to be confused with X86 terminology, where base pointer
/// refers to the frame pointer register) is a pointer an object. In opposition
/// to this, a derived pointer points to the interior of an object.
///
/// The Derived variant of a `PtrSlot` also contains a Stack Pointer offset to
/// the base of the object.
#[derive(Debug)]
enum PtrSlot {
    Base(SPO),
    Derived(SPO, SPO)
}

/// Contains root locations for a Safepoint.
#[derive(Debug)]
pub struct SafepointRoots {
    /// A list of registers which contain roots across a safepoint
    /// DWARF Register number mapping can be found here:
    /// Pg.63 https://software.intel.com/sites/default/files/article/402129/mpx-linux64-abi.pdf
    registers: Vec<u16>,

    /// A list of `PtrSlot`s which correspond to roots accessible from a stack
    /// pointer offset across a safepoint.
    stack_offsets: Vec<PtrSlot>
}

/// Converts an offset to always be from the Stack Pointer.
/// Pointer slots in stackmap locations are treated as offsets from a
/// particular register. In the case of a Direct or Indirect location kind,
/// these are the frame pointer and stack pointer registers respectively. To
/// avoid calculating this during a GC pause, we convert all offsets to be
/// from an SP upfront.
fn as_sp_offset(offset: &LocOffset) -> SPO {
    match offset {
        LocOffset::I32(ref o) => SPO(*o as u32),
        _ => panic!("Offset must be signed")
    }
}

fn gen_safepoint_roots(stackmap: SMRec) -> SafepointRoots {
    // The first 2 locations are uninteresting, however, they should be constants.
    debug_assert_eq!(
        mem::discriminant(&stackmap.locs[0].kind),
        mem::discriminant(&LocKind::Constant)
    );
    debug_assert_eq!(
        mem::discriminant(&stackmap.locs[1].kind),
        mem::discriminant(&LocKind::Constant)
    );
    let mut idx = NUM_SKIP_STACKMAPS;

    // The 3rd location specifies the number of de-opt locations. De-opt
    // params are not interesting to us, so we skip over them.
    let num_deopts = match stackmap.locs[idx].offset {
        LocOffset::U32(c) => c,
        _ => panic!("Constants must be u32")
    };
    idx += (num_deopts as usize) + 1;

    // The remaining indices in the loc vector should all be for GC pointers.
    // There should be 2 pointers in this list for each GC pointer in the
    // IR: a base pointer; and a derived pointer.
    //
    // We check that the number of remaining values is even.
    debug_assert!((stackmap.locs.len() - idx) % 2 == 0);
    let mut offsets = Vec::new();
    let mut gc_ptrs = stackmap.locs.iter().skip(idx);

    while let Some(base) = gc_ptrs.next() {
        let derived = gc_ptrs.next().unwrap();
        match base.kind {
            LocKind::Register => {
                eprintln!("UNIMPLEMENTED: Skipping Registers for now");
            }
            LocKind::Indirect => match derived.kind {
                LocKind::Indirect => {
                    if base.offset == derived.offset {
                        offsets.push(PtrSlot::Base(as_sp_offset(&base.offset)))
                    } else {
                        offsets.push(PtrSlot::Derived(
                            as_sp_offset(&base.offset),
                            as_sp_offset(&derived.offset)
                        ))
                    }
                }
                _ => unimplemented!()
            },
            _ => eprintln!("UNIMPLEMENTED: Skipping over value")
        }
    }

    SafepointRoots {
        registers: Vec::new(),
        stack_offsets: offsets
    }
}

/// Generates a safepoint table which can be used during GC to lookup
/// information about where pointers reside in a program.
///
/// This function will parse the .llvm_stackmap section of the given ELF file
/// and generate an efficient hashmap -- keyed by a function's return address --
/// which can be queried by the collector.
pub fn gen_safepoint_table<P: AsRef<Path>>(path: P) -> HashMap<ReturnAddress, SafepointRoots> {
    let parser = StackMapParser::new(path.as_ref()).unwrap();

    let mut frames = HashMap::new();
    let ref mut stackmaps = parser.iter_stackmaps();

    // Read functions
    for func in parser.iter_functions() {
        let func = func.unwrap();
        for sm in stackmaps.take(func.record_count() as usize) {
            frames.insert(ReturnAddress(func.addr()), gen_safepoint_roots(sm.unwrap()));
        }
    }
    frames
}
