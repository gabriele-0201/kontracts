#![cfg_attr(not(feature = "std"), no_std)]

use core::slice::from_raw_parts;

use sp_std::{collections::btree_map::BTreeMap, vec::Vec};

pub type Key = Vec<u8>;
pub type Value = Vec<u8>;
type RawValue = (*const u8, usize);
type RawKontractStorage = Vec<u8>;
type KontractStorage = BTreeMap<Key, Value>;

pub struct KontractStore {
	max_n_read: u32,
	max_n_write: u32,
	curr_n_read: u32,
	curr_n_write: u32,
	storage: KontractStorage,
}

#[cfg(feature = "std")]
mod kontracts_host_function {
	use super::{Key, KontractStorage, KontractStore, RawValue, Value};
	use codec::{Decode, Encode};
	use sp_core::{Blake2Hasher, Hasher};
	use wasmtime::{Caller, Trap};

	// false = read
	fn update_with_check(val: &mut u32, max: u32, r_or_w: bool) -> Result<(), Trap> {
		*val = match *val + 1 {
			x if x <= max => x,
			_ =>
				return Err(match r_or_w {
					true => Trap::new("ExceededWrites"),
					false => Trap::new("ExceededReads"),
				}),
		};
		Ok(())
	}

	// What I get here is raw bytes, probably already encoded by the wasm, but is not managed by the
	// storage
	pub fn kontracts_set(
		caller: &mut Caller<'_, KontractStore>,
		key: Key,
		value: Value,
	) -> Result<(), Trap> {
		//println!("KONTRACS: Instert new elem (key: {:?}, value: {:?})", key, value);

		let store = caller.data_mut();

		// Throw Trap if the number of write goes over the maximum supported number
		update_with_check(&mut store.curr_n_write, store.max_n_write, true)?;

		caller
			.data_mut()
			.storage
			.insert(Blake2Hasher::hash(&key[..])[..].to_vec(), value);

		Ok(())
	}

	pub fn kontracts_get(
		caller: &mut Caller<'_, KontractStore>,
		key: Key,
	) -> Result<Option<Value>, Trap> {
		let store = caller.data_mut();

		update_with_check(&mut store.curr_n_read, store.max_n_read, false)?;

		let value = store.storage.get(&Blake2Hasher::hash(&key.clone()[..])[..].to_vec());

		//println!("KONTRACS: Get elem (key: {:?}, value: {:?})", key, value);

		// TODO: here I have do to owned (that call clone underneath) to avoid managing lifetime...
		Ok(value.map(|v| v.to_owned()))
	}

	pub fn kontracts_remove(caller: &mut Caller<'_, KontractStore>, key: Key) -> Result<(), Trap> {
		let store = caller.data_mut();

		// Throw Trap if the number of write goes over the maximum supported number
		// To make thik easier I count the remove as a write
		update_with_check(&mut store.curr_n_write, store.max_n_write, true)?;

		//println!("KONTRACS: Get key: {:?} to remove", key);

		store.storage.remove(&Blake2Hasher::hash(&key[..])[..].to_vec());
		Ok(())
	}

	pub fn read_vec(
		caller: &mut Caller<'_, KontractStore>,
		ptr: u32,
		size: u32,
	) -> Result<Value, Trap> {
		let mem = match caller.get_export("memory") {
			Some(wasmtime::Extern::Memory(mem)) => mem,
			_ => return Err(Trap::new("Impossible reading wasm memory")),
		};

		// Use the `ptr` and `len` values to get a subslice of the wasm-memory
		let wasm_slice: Option<&[u8]> = mem
			.data(&caller)
			.get(ptr as u32 as usize..)
			.and_then(|arr| arr.get(..size as u32 as usize));

		match wasm_slice {
			Some(w) => Ok(w.to_vec()),
			None => Err(Trap::new("Impossible reading wasm memory")),
		}
	}

	pub fn write_vec(
		caller: &mut Caller<'_, KontractStore>,
		vec: Vec<u8>,
		ptr: u32,
		size: u32,
	) -> Result<(), Trap> {
		let mem = match caller.get_export("memory") {
			Some(wasmtime::Extern::Memory(mem)) => mem,
			_ => return Err(Trap::new("Impossible write data in wasm memory")),
		};

		if (size as usize) < vec.len() {
			return Err(Trap::new("Impossible write data in wasm memory"))
		}

		let wasm_buffer = &mut mem.data_mut(caller)[ptr as usize..=ptr as usize + vec.len()];

		wasm_buffer.copy_from_slice(&vec.encode()[..]);

		Ok(())
	}
}

// I do not like this design choise,
// maybe is better to have two different error:
// + here
// + in the pallet
// and the implement a from method, this could also be used to create a
// surjective conversion
// Another problem of this approch is that now I need two new dependencies: frame_support and
// scale_info
#[derive(
	codec::Encode,
	codec::Decode,
	frame_support::PalletError,
	frame_support::pallet_prelude::TypeInfo,
)]
pub enum ExecutionErrors {
	IncorrecBinary,
	ImpossibleCreateInstance, // Not sure why this happen
	MainEntryPointNotDefined,
	WasmPanic,
	ImpossibleAddFuel,
	ImpossibleCreateEngine,
	ImpossibleCreateHostFunction,
	ImpossibleDecodingKontractStorage,
	UnexpectedBehavoiur,
	OutOfFuel,
	OutOfReads,
	OutOfWrites,
}

#[sp_runtime_interface::runtime_interface]
pub trait KontractsExecutor {
	fn execute_code(
		&mut self,
		code: Vec<u8>,
		storage: RawKontractStorage,
		fuel: u32,
		max_read: u32,
		max_write: u32,
	) -> Result<RawKontractStorage, ExecutionErrors> {
		//println!("Entered in the KontractExecutor");

		use codec::{Decode, Encode};
		use wasmtime::*;

		let engine = Engine::new(Config::new().consume_fuel(true))
			.map_err(|_| ExecutionErrors::ImpossibleCreateEngine)?;

		let module = Module::new(&engine, code).map_err(|_| ExecutionErrors::IncorrecBinary)?;

		let mut store = Store::new(
			&engine,
			KontractStore {
				max_n_read: max_read,
				max_n_write: max_write,
				curr_n_read: 0,
				curr_n_write: 0,
				storage: Decode::decode(&mut &storage[..])
					.map_err(|_| ExecutionErrors::ImpossibleDecodingKontractStorage)?,
			},
		);

		store.add_fuel(fuel as u64).map_err(|_| ExecutionErrors::ImpossibleAddFuel)?;

		let mut linker = Linker::new(&engine);

		linker
			.func_wrap(
				"env",
				"set",
				|mut caller: Caller<'_, KontractStore>,
				 key_ptr: u32,
				 key_size: u32,
				 value_ptr: u32,
				 value_size: u32|
				 -> Result<(), Trap> {
					// The inputs are the pointers the begining of the vec
					// Those are u32 because the wasm executor work in 32bit
					// to create a real sandbox execution
					//
					// I think that could be added some sort of mememory error handling,
					// 100% I'm forgetting something

					let key_vec =
						kontracts_host_function::read_vec(&mut caller, key_ptr, key_size)?;
					let value_vec =
						kontracts_host_function::read_vec(&mut caller, value_ptr, value_size)?;

					//println!("Key Vec: {:?}", key_vec);
					//println!("Value Vec: {:?}", value_vec);

					kontracts_host_function::kontracts_set(&mut caller, key_vec, value_vec)
				},
			)
			.map_err(|_| ExecutionErrors::ImpossibleDecodingKontractStorage)?;

		linker
			.func_wrap(
				"env",
				"get",
				|mut caller: Caller<'_, KontractStore>,
				 key_ptr: u32,
				 key_size: u32,
				 value_ptr: u32,
				 value_max_size: u32|
				 -> Result<(), Trap> {
					let key_vec =
						kontracts_host_function::read_vec(&mut caller, key_ptr, key_size)?;

					//println!("Key Vec: {:?}", key_vec);

					// I think this is no a really good approch, no enough time to do a better one:
					// If the key is not present in the storage than I use an empty vec, write it
					// in the wasm buffer so that in the other side the initial of the buffer is a
					// compact encoding 0 => empty vec = no value
					let value_vec = kontracts_host_function::kontracts_get(&mut caller, key_vec)?
						.unwrap_or(vec![]);

					//println!("Key Vec From the storage: {:?}", value_vec);

					kontracts_host_function::write_vec(
						&mut caller,
						value_vec,
						value_ptr,
						value_max_size,
					)
				},
			)
			.map_err(|_| ExecutionErrors::ImpossibleDecodingKontractStorage)?;

		linker
			.func_wrap(
				"env",
				"remove",
				|mut caller: Caller<'_, KontractStore>,
				 key_ptr: u32,
				 key_size: u32|
				 -> Result<(), Trap> {
					let key_vec =
						kontracts_host_function::read_vec(&mut caller, key_ptr, key_size)?;

					kontracts_host_function::kontracts_remove(&mut caller, key_vec)
				},
			)
			.map_err(|_| ExecutionErrors::ImpossibleDecodingKontractStorage)?;

		let instance = linker
			.instantiate(&mut store, &module)
			.map_err(|_| ExecutionErrors::ImpossibleCreateInstance)?;

		let main = instance
			.get_typed_func::<(), (), _>(&mut store, "main")
			.map_err(|_| ExecutionErrors::MainEntryPointNotDefined)?;

		// And finally we can call the wasm!
		// Here there is a GIGANTIC generalization...
		// not all TRAPs are equal to panic but with only 4 days I don't have time to
		// manage all of them.... Trap = Panic (removed my own traps)
		main.call(&mut store, ()).map_err(|trap| {
			match format!("{}", trap.display_reason()).as_str() {
				"all fuel consumed by WebAssembly" => ExecutionErrors::OutOfFuel,
				"ExceededReads" => ExecutionErrors::OutOfReads,
				"ExceededWrites" => ExecutionErrors::OutOfWrites,
				_ => ExecutionErrors::WasmPanic,
			}
		})?;

		// println!("{:?}", store.data());

		Ok(store.data().storage.encode())
	}
}
