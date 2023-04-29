#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(core_intrinsics)]

#[kontracts_proc_macro::kontracts]
fn main() {
    for _ in 0..100 {}
}
