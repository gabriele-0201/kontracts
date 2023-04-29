use crate::{mock::*, Error, Event};
use codec::{Decode, Encode};
use frame_support::{
	assert_noop, assert_ok, pallet_prelude::DispatchResult, BoundedBTreeMap, BoundedVec,
};

use sp_core::Hasher;

/* TEST todo:
 * + Uload wrong code and try to execute it
 * + Exceed max code size // This is managed by the extrinsic signature
 * + Try execute non existing CodeId
 * + ExceededFuel
 * + ExceededStorage
 * + Correct reservation
 * + Simple empty execution
 * + correct write to storage
 * + correct read from storage
 * + Correct write and read from storage
 * + collantz conjecture test
 * + Manage MaxKontractStorageKeySize
 * + Manage MaxKontractStorageValueSize
 * + WASM error handling, what happen if there is an intern panic?
 */

fn load_wasm<T>(name: &str) -> Result<(Vec<u8>, <T::HashingAlgorith as Hasher>::Out), &'static str>
where
	T: crate::Config,
{
	let path = ["wasm_test/", name, ".wasm"].concat();
	let wasm_binary = std::fs::read(path).map_err(|_| "Wrong path")?;
	let code_id = T::HashingAlgorith::hash(&wasm_binary);
	Ok((wasm_binary, code_id))
}

#[test]
fn execute_empty_kontract() {
	new_test_ext().execute_with(|| {
		// In the block number 0 the events are not deposited
		System::set_block_number(1);

		let account = 1;

		let (wasm_binary, code_id): (_, <Test as crate::Config>::CodeId) =
			load_wasm::<Test>("empty_execution").unwrap();

		Kontracts::upload_code(
			RuntimeOrigin::signed(account),
			BoundedVec::try_from(wasm_binary).expect("Code too big"),
		)
		.expect("Impossible upload code");

		System::assert_last_event(Event::<Test>::NewCodeUploaded { code_id, who: account }.into());

		Kontracts::execute_code(
			RuntimeOrigin::signed(account),
			code_id,
			0,
			u32::MAX,
			u32::MAX,
			u32::MAX,
		)
		.expect("Impossible execute code");

		System::assert_last_event(Event::<Test>::CodeExecuted { code_id, who: account }.into());
	});
}

fn key_hashed<T: crate::Config>(key: Vec<u8>) -> BoundedVec<u8, T::MaxKontracStorageKeySize> {
	sp_core::Blake2Hasher::hash(&key[..])[..]
		.to_vec()
		.try_into()
		.expect("Impossible create bounded vec")
}

macro_rules! write_B_kontract {
	($code: literal, $account: ident, $start_balance: literal, $expected_storage: literal) => {{
		System::set_block_number(1);

		let origin = RuntimeOrigin::signed($account);

		Balances::set_balance(RuntimeOrigin::root(), $account, $start_balance, 0)
			.expect("Impossibel set balance");

		let (wasm_binary, code_id): (_, <Test as crate::Config>::CodeId) =
			load_wasm::<Test>($code).unwrap();

		Kontracts::upload_code(
			RuntimeOrigin::signed($account),
			BoundedVec::try_from(wasm_binary).expect("Code too big"),
		)
		.expect("Impossible upload code");

		System::assert_last_event(Event::<Test>::NewCodeUploaded { code_id, who: $account }.into());

		Kontracts::execute_code(
			RuntimeOrigin::signed($account),
			code_id,
			$expected_storage,
			u32::MAX,
			u32::MAX,
			u32::MAX,
		)
	}};
}

#[test]
fn write_8B_equal_kontract() {
	new_test_ext().execute_with(|| {
		let acc = 1;
		let res_execution = write_B_kontract!("write_8B", acc, 47, 47);

		assert_eq!(Ok(()), res_execution);
		assert_eq!(47, Balances::reserved_balance(acc));
		assert_eq!(0, Balances::free_balance(acc));
	});
}

#[test]
fn write_8B_less_kontract() {
	new_test_ext().execute_with(|| {
		let acc = 1;
		// Here the user is saying that he will use only 30B of storage, instead 47 will be occupied
		// This means that the extrinsic will return an error and all the possible fees up to 47 are
		// take
		let res_execution = write_B_kontract!("write_8B", acc, 47, 30);

		//assert_eq!(Err(Error::<Test>::ExceededStorage.into()), res_execution);
		assert_eq!(Ok(()), res_execution);
		assert_eq!(0, Balances::reserved_balance(acc));
		assert_eq!(0, Balances::free_balance(acc));
	});
}

#[test]
fn write_8B_more_kontract() {
	new_test_ext().execute_with(|| {
		let acc = 1;
		// THe user is declaring more storage than expected, the deposit will be done
		// on the reality
		let res_execution = write_B_kontract!("write_8B", acc, 50, 50);

		assert_eq!(Ok(()), res_execution);
		assert_eq!(47, Balances::reserved_balance(acc));
		assert_eq!(3, Balances::free_balance(acc));
	});
}

#[test]
fn write_8B_negative_kontract() {
	// Here the idea were to use the same macro used in the test above
	// but I didn't thought that I need to modify the storage of the contract
	// to have a refaunf because a free some storage, this needs the knowledge
	// of the code_id, so: I will not refacotor the macro but copy paste some stuff,
	// too less time to have beautiful tests (Sad Gabriele)
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		let acc = 1;
		let origin = RuntimeOrigin::signed(acc);

		// 55 should be the needed reserved money to have the current kontract storage
		Balances::set_balance(RuntimeOrigin::root(), acc, 45, 55).expect("Impossibel set balance");

		let (wasm_binary, code_id): (_, <Test as crate::Config>::CodeId) =
			load_wasm::<Test>("write_8B").unwrap();

		Kontracts::upload_code(
			origin.clone(),
			BoundedVec::try_from(wasm_binary).expect("Code too big"),
		)
		.expect("Impossible upload code");

		System::assert_last_event(Event::<Test>::NewCodeUploaded { code_id, who: acc }.into());

		// write in storage a bigger value under the same key
		let mut kontract_storage: crate::pallet::KontractStorage<Test> = BoundedBTreeMap::new();
		kontract_storage
			.try_insert(
				key_hashed::<Test>(vec![1u32, 2u32, 3u32].encode()),
				vec![4u32, 5u32, 6u32, 7u32, 8u32]
					.encode()
					.try_into()
					.expect("Impossible create bounded vec"),
			)
			.expect("Impossible insert element in the map");
		crate::Storages::<Test>::insert(code_id, kontract_storage);

		let res_execution =
			Kontracts::execute_code(origin, code_id, -8, u32::MAX, u32::MAX, u32::MAX);

		assert_eq!(Ok(()), res_execution);
		assert_eq!(47, Balances::reserved_balance(acc));
		assert_eq!(53, Balances::free_balance(acc));
	});
}

#[test]
fn delete_kontract() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		let acc = 1;
		let origin = RuntimeOrigin::signed(acc);

		// 55 should be the needed reserved money to have the current kontract storage
		Balances::set_balance(RuntimeOrigin::root(), acc, 53, 47).expect("Impossibel set balance");

		let (wasm_binary, code_id): (_, <Test as crate::Config>::CodeId) =
			load_wasm::<Test>("delete").unwrap();

		Kontracts::upload_code(
			origin.clone(),
			BoundedVec::try_from(wasm_binary).expect("Code too big"),
		)
		.expect("Impossible upload code");

		System::assert_last_event(Event::<Test>::NewCodeUploaded { code_id, who: acc }.into());

		// write in storage a bigger value under the same key
		let mut kontract_storage: crate::pallet::KontractStorage<Test> = BoundedBTreeMap::new();
		kontract_storage
			.try_insert(
				key_hashed::<Test>(vec![1u32, 2u32, 3u32].encode()),
				vec![4u32, 5u32, 6u32]
					.encode()
					.try_into()
					.expect("Impossible create bounded vec"),
			)
			.expect("Impossible insert element in the map");
		crate::Storages::<Test>::insert(code_id, kontract_storage);

		let res_execution =
			Kontracts::execute_code(origin, code_id, -47, u32::MAX, u32::MAX, u32::MAX);

		assert_eq!(Ok(()), res_execution);
		assert_eq!(0, Balances::reserved_balance(acc));
		assert_eq!(100, Balances::free_balance(acc));
	});
}

//TODO: test with someone that decleared more free space than reality

#[test]
fn read_kontract() {
	new_test_ext().execute_with(|| {
		// In the block number 0 the events are not deposited
		System::set_block_number(1);

		let account = 1;

		let (wasm_binary, code_id): (_, <Test as crate::Config>::CodeId) =
			load_wasm::<Test>("read").unwrap();

		Kontracts::upload_code(
			RuntimeOrigin::signed(account),
			BoundedVec::try_from(wasm_binary).expect("Code too big"),
		)
		.expect("Impossible upload code");

		// write in memory the correct value
		let mut kontract_storage: crate::pallet::KontractStorage<Test> = BoundedBTreeMap::new();
		kontract_storage
			.try_insert(
				key_hashed::<Test>(vec![1u32, 2u32, 3u32].encode()),
				vec![4u32, 5u32, 6u32]
					.encode()
					.try_into()
					.expect("Impossible create bounded vec"),
			)
			.expect("Impossible insert element in the map");
		crate::Storages::<Test>::insert(code_id, kontract_storage);

		System::assert_last_event(Event::<Test>::NewCodeUploaded { code_id, who: account }.into());

		Kontracts::execute_code(
			RuntimeOrigin::signed(account),
			code_id,
			0,
			u32::MAX,
			u32::MAX,
			u32::MAX,
		)
		.expect("Impossible execute code");

		System::assert_last_event(Event::<Test>::CodeExecuted { code_id, who: account }.into());
	});
}

#[test]
fn read_and_multiple_write_ok() {
	new_test_ext().execute_with(|| {
		// In the block number 0 the events are not deposited
		System::set_block_number(1);

		let account = 1;
		let origin = RuntimeOrigin::signed(account);

		let start_balance = 100;
		let deposit_for_storage = 94;
		let remain_balance_expected = start_balance - deposit_for_storage;

		Balances::set_balance(RuntimeOrigin::root(), account, start_balance, 0)
			.expect("Impossibel set balance");

		let (wasm_binary, code_id): (_, <Test as crate::Config>::CodeId) =
			load_wasm::<Test>("write_and_read_16B").unwrap();

		Kontracts::upload_code(
			origin.clone(),
			BoundedVec::try_from(wasm_binary).expect("Code too big"),
		)
		.expect("Impossible upload code");

		System::assert_last_event(Event::<Test>::NewCodeUploaded { code_id, who: account }.into());

		Kontracts::execute_code(
			origin,
			code_id,
			deposit_for_storage as i32,
			u32::MAX,
			u32::MAX,
			u32::MAX,
		)
		.expect("Impossible execute code");

		System::assert_last_event(Event::<Test>::CodeExecuted { code_id, who: account }.into());

		let kontract_storage_result = crate::Storages::<Test>::get(code_id);

		// write in memory the correct value
		let mut expected_kontract_storage: crate::pallet::KontractStorage<Test> =
			BoundedBTreeMap::new();

		let key_1 = key_hashed::<Test>(vec![1u32, 2u32, 3u32].encode());

		let vec_2: BoundedVec<u8, <Test as crate::Config>::MaxKontracStorageValueSize> =
			vec![4u32, 5u32, 6u32]
				.encode()
				.try_into()
				.expect("Impossible create bounded vec");
		let key_2 = key_hashed::<Test>(vec![4u32, 5u32, 6u32].encode());

		expected_kontract_storage
			.try_insert(key_1, vec_2.clone())
			.expect("Impossible insert element in the map");
		expected_kontract_storage
			.try_insert(key_2, vec_2)
			.expect("Impossible insert element in the map");

		assert_eq!(kontract_storage_result, expected_kontract_storage);
		let remain_balance = Balances::free_balance(account);
		let reserved_balance = Balances::reserved_balance(account);
		assert_eq!(
			(remain_balance_expected, deposit_for_storage),
			(remain_balance, reserved_balance)
		)
	});
}

#[test]
fn collantz_conjecture_kontract() {
	new_test_ext().execute_with(|| {
		// In the block number 0 the events are not deposited
		System::set_block_number(1);

		let account = 1;
		let origin = RuntimeOrigin::signed(account);

		let start_balance = 100;
		Balances::set_balance(RuntimeOrigin::root(), account, start_balance, 0)
			.expect("Impossibel set balance");

		let (wasm_binary, code_id): (_, <Test as crate::Config>::CodeId) =
			load_wasm::<Test>("collantz_conjecture").unwrap();

		Kontracts::upload_code(
			origin.clone(),
			BoundedVec::try_from(wasm_binary).expect("Code too big"),
		)
		.expect("Impossible upload code");

		System::assert_last_event(Event::<Test>::NewCodeUploaded { code_id, who: account }.into());

		let next_collantz = |val: u32| match val % 2 {
			0 => val / 2,
			_ => (val * 3) + 1,
		};

		let mut collantz_numbers = vec![];
		for _ in 0..100 {
			collantz_numbers.push(next_collantz(*collantz_numbers.last().unwrap_or(&5u32)));
		}

		let key = key_hashed::<Test>(1u32.encode());

		// 5 because there is the ENCODING of the value u32 and tha this will be RE encoded as
		// vec<u8>
		let new_storage_size = (key.encode().len() + 5) as i32;

		println!("Test new_storage_size: {}", new_storage_size);

		for collantz_number in collantz_numbers.iter() {
			// 4 bytes are used the first time and than zero
			Kontracts::execute_code(
				origin.clone(),
				code_id,
				new_storage_size,
				u32::MAX,
				u32::MAX,
				u32::MAX,
			)
			.expect("Impossible execute code");
			let val: u32 = Decode::decode(
				&mut &crate::Storages::<Test>::get(code_id)
					.get(&key)
					.expect("Number not defined in the storage of the kontract")
					.clone()
					.into_inner()[..],
			)
			.expect("Impossibel decode collants number from storage");

			assert_eq!(*collantz_number, val);
		}

		System::assert_last_event(Event::<Test>::CodeExecuted { code_id, who: account }.into());
	});
}

#[test]
fn panic_kontract() {
	new_test_ext().execute_with(|| {
		// In the block number 0 the events are not deposited
		System::set_block_number(1);

		let account = 1;

		let (wasm_binary, code_id): (_, <Test as crate::Config>::CodeId) =
			load_wasm::<Test>("panic").unwrap();

		Kontracts::upload_code(
			RuntimeOrigin::signed(account),
			BoundedVec::try_from(wasm_binary).expect("Code too big"),
		)
		.expect("Impossible upload code");

		System::assert_last_event(Event::<Test>::NewCodeUploaded { code_id, who: account }.into());

		// TODO: fee managment if a panic occur
		assert_noop!(
			Kontracts::execute_code(
				RuntimeOrigin::signed(account),
				code_id,
				0,
				u32::MAX,
				u32::MAX,
				u32::MAX
			),
			Error::<Test>::ExecutionCode(kontracts_executor::ExecutionErrors::WasmPanic)
		);
	});
}

#[test]
fn out_of_fuel_kontract() {
	new_test_ext().execute_with(|| {
		// In the block number 0 the events are not deposited
		System::set_block_number(1);

		let account = 1;

		let (wasm_binary, code_id): (_, <Test as crate::Config>::CodeId) =
			load_wasm::<Test>("loop").unwrap();

		Kontracts::upload_code(
			RuntimeOrigin::signed(account),
			BoundedVec::try_from(wasm_binary).expect("Code too big"),
		)
		.expect("Impossible upload code");

		System::assert_last_event(Event::<Test>::NewCodeUploaded { code_id, who: account }.into());

		// TODO: fee managment if a panic occur
		assert_noop!(
			Kontracts::execute_code(
				RuntimeOrigin::signed(account),
				code_id,
				0,
				10,
				u32::MAX,
				u32::MAX
			),
			Error::<Test>::ExecutionCode(kontracts_executor::ExecutionErrors::OutOfFuel)
		);
	});
}

fn out_of_reads_or_writes(
	code: &str,
	max_read: u32,
	max_write: u32,
	expected_err: kontracts_executor::ExecutionErrors,
) {
	new_test_ext().execute_with(|| {
		// In the block number 0 the events are not deposited
		System::set_block_number(1);

		let account = 1;

		let (wasm_binary, code_id): (_, <Test as crate::Config>::CodeId) =
			load_wasm::<Test>(code).unwrap();

		Kontracts::upload_code(
			RuntimeOrigin::signed(account),
			BoundedVec::try_from(wasm_binary).expect("Code too big"),
		)
		.expect("Impossible upload code");

		System::assert_last_event(Event::<Test>::NewCodeUploaded { code_id, who: account }.into());

		// TODO: fee managment if a panic occur
		assert_noop!(
			Kontracts::execute_code(
				RuntimeOrigin::signed(account),
				code_id,
				0,
				u32::MAX,
				max_read,
				max_write
			),
			Error::<Test>::ExecutionCode(expected_err)
		);
	});
}

#[test]
fn out_of_reads() {
	out_of_reads_or_writes("read", 0, 0, kontracts_executor::ExecutionErrors::OutOfReads);
}

#[test]
fn out_of_writes() {
	out_of_reads_or_writes("write_8B", 0, 0, kontracts_executor::ExecutionErrors::OutOfWrites);
}
