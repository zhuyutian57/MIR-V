use std::{alloc::{alloc, dealloc, Layout}, ptr};

struct TData {
    lo : *mut u8,
    hi : *mut u8,
}

fn alloc_data(pdata : &mut TData) {
    pdata.lo = unsafe { alloc(Layout::new::<u16>()) };
    pdata.hi = unsafe { alloc(Layout::new::<[u8; 3]>()) };
}

fn free_data(mut data : TData) {
    let lo = data.lo;
    let hi = data.hi;
    
    if lo == hi {
        unsafe {
            dealloc(lo, Layout::new::<u16>());
            dealloc(hi, Layout::new::<[u8; 3]>());
        }
    }

    data.lo = ptr::null_mut();
    data.hi = ptr::null_mut();
}

fn main() {
    let mut data = TData {
        lo : ptr::null_mut() as *mut u8,
        hi : ptr::null_mut() as *mut u8,
    };
    alloc_data(&mut data);
    free_data(data);

    // memory-leak
}