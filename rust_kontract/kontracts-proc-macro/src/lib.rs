#![crate_type = "proc-macro"]
extern crate proc_macro;
use proc_macro::TokenStream;
use quote::{quote, ToTokens};

#[proc_macro_attribute]
pub fn kontracts(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let main_function: syn::ItemFn = syn::parse(input).unwrap();
    TokenStream::from(quote!(

        #[panic_handler]
        fn panic(_info: &core::panic::PanicInfo) -> ! {
            unsafe {
                core::intrinsics::abort();
            }
            loop {}
        }

        // Use `wee_alloc` as the global allocator.
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

        #[macro_use]
        extern crate alloc;

        type Key = alloc::vec::Vec<u8>;
        type Value = alloc::vec::Vec<u8>;
        const MaxValueBytes: u32 = 100;

        extern "C" {
            pub fn set(key_ptr: u32, key_size: u32, value_ptr: u32, value_size: u32);
            pub fn get(key_ptr: u32, key_size: u32, value_ptr: u32, value_max_size: u32);
            pub fn remove(key_ptr: u32, key_size: u32);
        }

        fn write(key: Key, value: Value) {
            unsafe {
                set(
                    key[..].as_ptr() as u32,
                    key.len() as u32,
                    value[..].as_ptr() as u32,
                    value.len() as u32,
                );
            }
        }

        fn read(key: Key) -> alloc::vec::Vec<u8> {
            let result = vec![0; MaxValueBytes as usize];
            unsafe {
                get(
                    key[..].as_ptr() as u32,
                    key.len() as u32,
                    result[..].as_ptr() as u32,
                    result.len() as u32,
                );
            }
            parity_scale_codec::Decode::decode(&mut &result[..]).expect("Result is not decodable as Vec<u8>")
        }

        fn delete(key: Key) {
            unsafe {
                remove(
                    key[..].as_ptr() as u32,
                    key.len() as u32
                );
            }
        }

        #[no_mangle]
        #main_function
    ))
}
