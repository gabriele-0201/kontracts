# Kontracts

This is modified substrate node (just a new host function) that make possible to execute kontracts with wasmtime (an env. parallel to the runtime) and not using wasmi (what substrate uses right now), everything managed through a custom pallet.

## What's a kontract?

A simple wasm binary with three contraint:
+ There are 3 defined host functions:

    + get(key_ptr: i32, key_size: i32, result_ptr: i32, result_max_size:i32)
    + set(key_ptr: i32, key_size: i32, value_ptr: i32, value_max_size:i32)
    + set(key_ptr: i32, key_size: i32)
    
    Where each couple ptr and size is used to make possible the comunication between
    wasm and the client following the FFI
+ Single entry point: `main`
+ No input and no output 

(I would really love in the future to also add the possibility of have multiple 
entry point and also input and output arguments, this would greatly increase 
the potential of kontracts)

### So what can to a kontract?

A kontract has access to his own storage inside the chain, the type of storage 
is a `Map<Vec<u8>, Vec<u8>>`, so every contract can write, delete and read from
his storage everything, as soon as coul be rappresented with bytes.

Now is up to you write a contract that does interesting thing (considering the 
restrictions explained after), an example could be the `Collatz Conjecture Kontract`, 
this will write on the storage under a specifinc key the scale encode rappresentation 
of the next number of this series.

### What can't do a kontract

Every kontract is restricted to his own storage and can't interact with others,
also there is some limitation that are dynamically decided by the user:
+ Fuel
    every kontract execution the user can specify how much fuel the kontract 
    can consume, there is almost a direct connection between 1 unif of fuel
    and 1 wasm instruction (there are some istruction that consume 0 fuel)
+ Expected read and writes
    Every kontract has also a bounded number of reads and write of the storage,
    those number are up to the user that will call the kontract

### How can I develop a kontract?

I implemented a little proc macro that enable you to write kontracts in a really
easy way, an example of the `Collatz Conjecture Kontract`:

```rust
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
```

As you can see before the proc-macro there are some directive, those should be
inside the proc macro but I faced a weird issue with `#![no_std]` inside proc-macro
and I didn't have time to figure out why it wasn't possible.

A consequetially constraint due to this way to build the kontracts is the 
non presence of std.

All of this is inside the folder: `rust_kontract`

## PalletKontract

All the substrate-node-template is under the `kontracs`

### Extrinsics

+ upload_code
    + Main behavior:
        + This extrinsic gives the possibilit to upload a kontract onchain, this
        will save the kontract and the relative id (hash of the binary) in the a
        storage map called: Codes
    + Fees:
        + The fees are directly proportional to the length of the wasm binary
    + Deposited Events:
        + NewCodeUploaded
    + Possible Errors:
        + BadOrigin

+ execute_code
    + Main behavior:
        + With this extrinsic is possible to execute the kontract but providing some informations:
            + code_id, self explanatory
            + expected_modified_storage, this is the variation of the storage (in bytes) after the execution
                => A positive number means that I will use new space, and for that the user need to deposit the same amount of balance
                => Instead a newgative  number means that some space will be freed up, than the consequence is the return of the the same amount of deposit
                Here the user can behave correctly => specifing more used space than the reality the amount used for the deposit is the real one
                If, on the other hand, less is said than expected the user will be slash for the real storage variation
            + fuel, this is the number of instruction that the kontract is able to execute
            + expected_read, this is the maximum number of read from the storage that the kontract is allowed to do, if it is exceeded than the extrinsic will fail
            + expected_write, same as expected_read but with the writes on the storage
    + Fees:
        + Directly proportional to the number of read, writes and also the quantity of fuel provided
    + Deposited Events:
        + ExceededStorage -> this means that the user got slashed
        + CodeExecuted
    + Possible Errors:
        + InvalidCodeId
        + ExecutionCode(ExecutionErrors)
            where ExecutionErrors can be: OutOfFuel, OutOfReads, OutOfWrites, WasmPanic and UnexpectedBehavoiur (When the returned storage is no longer decodable)
            (In reality those are not all the possible errors... there is a big generalization on WasmPanic)
        + DepositError => the user is not able to pay the deposit

+ delete_code
    + Main behavior:
        The root is able to delete codes
    + Fees:
        0 for now...
    + Deposited Events:
        + CodeDeleted

+ update_code
    + Main behavior:
        The root is able to delete a code and replace it with another
    + Fees:
        + The fees are directly proportional to the length of the new wasm binary
    + Deposited Events:
        + CodeUpgraded

## How the Kontract is executed?

The pallet is able to execute the wasm binary using a new host function:
`KontractExecutor::execute_code`, this function will (using wasmtime) crate
a new wasm context and provide to the new context the three host function
described at the beginning

+ some sort of trap inside wasm handling

## What I learned?

TOO MANY THINGS, doing this project was amazing

Unfortunately, figuring out how to implement a host function and then how to make the two worlds,
wasm and client, communicate, took me a lot of time and as a result I could not do a complete project 
with all edge cases covered and especially with suboptimal code quality
