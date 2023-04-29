#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

#[kontracts_proc_macro::kontracts]
fn main() {
	use alloc::vec;
	use parity_scale_codec::Encode;

	let key = vec![1u32, 2u32, 3u32];
	let value = vec![4u32, 5u32, 6u32];

	write(key.encode(), value.encode());
}
