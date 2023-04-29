cargo clean && cargo build --release && wasm-opt -Oz -o output.wasm target/wasm32-unknown-unknown/release/kontracs_executor.wasm && ls -l output.wasm
