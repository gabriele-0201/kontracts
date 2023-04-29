#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(core_intrinsics)]

#[kontracts_proc_macro::kontracts]
fn main() {
    use alloc::vec;
    use parity_scale_codec::Encode;

    let key = vec![1u32, 2u32, 3u32];

    delete(key.encode());
}
