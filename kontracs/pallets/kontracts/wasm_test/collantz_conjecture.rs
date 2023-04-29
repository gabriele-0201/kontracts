#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(core_intrinsics)]

#[kontracts_proc_macro::kontracts]
fn main() {
    use parity_scale_codec::{Decode, Encode};

    // Collatz Conjecture
    let key = 1u32;

    let val: u32 = Decode::decode(&mut &read(key.encode())[..]).unwrap_or(5);

    let res = match val % 2 {
        0 => val / 2,
        _ => (val * 3) + 1,
    };

    write(key.encode(), res.encode());
}
