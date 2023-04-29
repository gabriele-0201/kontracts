#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use codec::{Decode, Encode, EncodeLike, FullCodec};
	use frame_support::{
		dispatch::MaxEncodedLen,
		inherent::Vec,
		pallet_prelude::*,
		traits::{Currency, ReservableCurrency},
		BoundedBTreeMap, BoundedVec,
	};
	use frame_system::pallet_prelude::*;
	use kontracts_executor::{kontracts_executor::*, ExecutionErrors};
	use sp_core::Hasher;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: Currency<<Self as frame_system::Config>::AccountId>
			+ ReservableCurrency<<Self as frame_system::Config>::AccountId>;

        /// Max number of contracts that can be managed in the pallet
		type MaxNumberContracts: Get<u32>;

		/// Max size of the storage value, in Byte
		type MaxKontracStorageValueSize: Get<u32>;

		/// Max size of the storage key, in Byte
		type MaxKontracStorageKeySize: Get<u32>;

		/// Max size of the storage, in Byte
		type MaxKontracStorageSize: Get<u32>;

		/// Max code size, in Byte
		type MaxCodeSize: Get<u32>;

        /// Identifier of the code
		type CodeId: Decode
			+ FullCodec
			+ Clone
			+ Eq
			+ MaxEncodedLen
			+ TypeInfo
            + frame_support::dispatch::fmt::Debug;

        /// Hashing Algorith used to evaluate the code id based on the wasm binary
		type HashingAlgorith: Hasher<Out = Self::CodeId>;
	}

	pub type AccountId<T> = <T as frame_system::Config>::AccountId;
	pub type KontractStorage<T> = BoundedBTreeMap<
		BoundedVec<u8, <T as Config>::MaxKontracStorageKeySize>,
		BoundedVec<u8, <T as Config>::MaxKontracStorageValueSize>,
		<T as Config>::MaxKontracStorageSize,
	>;

	#[pallet::storage]
	pub type Codes<T> =
		StorageMap<_, Identity, <T as Config>::CodeId, BoundedVec<u8, <T as Config>::MaxCodeSize>>;

	#[pallet::storage]
	pub type Storages<T> =
		StorageMap<_, Identity, <T as Config>::CodeId, KontractStorage<T>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New code has been uploaded
		NewCodeUploaded { code_id: T::CodeId, who: AccountId<T> },

		/// Code has been executed
		CodeExecuted { code_id: T::CodeId, who: AccountId<T> },

		/// Code deleted by the root
		CodeDeleted { code_id: T::CodeId },

		/// Code upgrated by the root
		CodeUpgraded { old_code_id: T::CodeId, new_code_id: T::CodeId },

		/// Exceeded expected storage use
		ExceededStorage { code_id: T::CodeId, who: AccountId<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid CodeId
		InvalidCodeId,
		/// ExectionCodeError
		ExecutionCode(ExecutionErrors),
		/// Exceeded expected Fuel
		ExceededFuel,
		/// Invalid storage managment
		WrongStorageEncoding,
		/// Impossible accomplish a proper deposit for the storage usage
		DepositError,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		// 155 is a random value, this could be evaluated througth benchmarking
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time() + 155 * code.len() as u64)]
		pub fn upload_code(
			origin: OriginFor<T>,
			code: BoundedVec<u8, T::MaxCodeSize>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// TODO: some pre evaluation of the code I'm trying to save on chain

			let code_id = T::HashingAlgorith::hash(&code[..]);
			<Codes<T>>::insert(code_id.clone(), code);

			Self::deposit_event(Event::NewCodeUploaded { code_id, who });
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(
            10_000 + 
            T::DbWeight::get().reads_writes(*expected_read as u64, *expected_write as u64).ref_time() + 
            100 * *fuel as u64
            )]
		pub fn execute_code(
			origin: OriginFor<T>,
			code_id: T::CodeId,
			expected_modified_storage: i32,
			fuel: u32,
			expected_read: u32,
			expected_write: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let get_storage_size = |storage_encoded: &Vec<u8>| -> i32 {
				let mut ref_slice_vec = &storage_encoded[..];
				let _compact_len: codec::Compact<u32> =
					Decode::decode(&mut ref_slice_vec).expect("LLLLOOLLL");
				ref_slice_vec.len() as i32
			};

			let code = <Codes<T>>::get(code_id.clone()).ok_or(<Error<T>>::InvalidCodeId)?;

			let old_storage_raw = <Storages<T>>::get(code_id.clone()).encode();
			let old_storage_size = get_storage_size(&old_storage_raw);

			// I can easily encode the Storage because the encoding of a BoundedVec and
			// a vec is the same, I can so encode here from BoundedVec and than in the client
			// decode as Vec
			let new_storage_raw =
				execute_code(code.to_vec(), old_storage_raw, fuel, expected_read, expected_write)
					.map_err(|e| <Error<T>>::ExecutionCode(e))?;
			let new_storage_size = get_storage_size(&new_storage_raw);

			// Here I was doing the difference between two scale encoded vector but
			// is not so easy.. the user should manage also the change of the compact
			// encode in front of the BoundedBTreeMap, in the real life this make sense
			// but I don't have time to make a proper test suite and is easier work only with the
			// encoding of the entries of the storage
			//
			// Here there are a lot of not covered edge cases...
			match new_storage_size - old_storage_size {
				0 => (),
				// The kontract free x space so I have to unreserve some balance
				x if x <= expected_modified_storage && x < 0 =>
				// Here I don't care if the user has less balance than expected to unreserve
				{
					// I don't like this syntax.... the problem is that this arm return something
					// and if I want to return () than I need to add ';' but than also the {} are
					// required
					<T as Config>::Currency::unreserve(&who, ((x * -1) as u32).into());
				},
				// The kontract used x space, I have to reserve the same amount
				x if x <= expected_modified_storage =>
					<T as Config>::Currency::reserve(&who, (x as u32).into())
						.map_err(|_| <Error<T>>::DepositError)?,
				// The used space is more the expected, return Error and slash the account with the
				// same amount of new storage not correctly decleared
				x => {
					<T as Config>::Currency::slash(&who, (x as u32).into());
					// lol, that's true, I can't return error if I want to slash someone...
					// I will deposit an event of ExceededStorage and return Ok()
					// otherwise the overlay will be not applayed, I think there is 100% a better
					// solution
					Self::deposit_event(Event::ExceededStorage { code_id, who });
					return Ok(())
				},
			};

			// This Error should never happend but maybe the user find a way to break the storage of
			// a contract
			let storage: KontractStorage<T> = Decode::decode(&mut &new_storage_raw[..])
				.map_err(|_| <Error<T>>::ExecutionCode(ExecutionErrors::UnexpectedBehavoiur))?;
			<Storages<T>>::insert(code_id.clone(), storage);

			Self::deposit_event(Event::CodeExecuted { code_id, who });

            // TODO: DispatchResultWithPostInfo
            // If the user specify more read and write than the reallity than
            // a refund should be made

			Ok(())
		}

		#[pallet::call_index(2)]
        #[pallet::weight(0)]
		pub fn delete_code(
			origin: OriginFor<T>,
			code_id: T::CodeId,
		) -> DispatchResult {
			ensure_root(origin)?;

			<Codes<T>>::remove(code_id.clone());

			Self::deposit_event(Event::CodeDeleted { code_id });
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time() + 155 * code.len() as u64)]
		pub fn update_code(
			origin: OriginFor<T>,
			old_code_id: T::CodeId,
			code: BoundedVec<u8, T::MaxCodeSize>,
		) -> DispatchResult {
            // TODO: ensure root
			ensure_root(origin)?;

			<Codes<T>>::remove(old_code_id.clone());

			let new_code_id = T::HashingAlgorith::hash(&code[..]);
			<Codes<T>>::insert(new_code_id.clone(), code);

			Self::deposit_event(Event::CodeUpgraded { old_code_id, new_code_id });
			Ok(())
		}
	}
}
