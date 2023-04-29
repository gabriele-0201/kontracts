#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(core_intrinsics)]

#[kontracts_proc_macro::kontracts]
fn main() {
    use alloc::vec::Vec;
    use parity_scale_codec::{Decode, Encode};

    let key = vec![1u32, 2u32, 3u32];
    let value = vec![4u32, 5u32, 6u32];

    write(key.encode(), value.encode());

    let read_value: Vec<u32> =
        Decode::decode(&mut &read(key.encode())[..]).expect("Impossible decode whats' I insered");

    let key_2 = vec![4u32, 5u32, 6u32];

    write(key_2.encode(), read_value.encode());
}
