//! Logs one line. Built with `cargo build-sbf` into `tests/programs/hello.so`.

#[no_mangle]
pub extern "C" fn entrypoint(_input: *mut u8) -> u64 {
    solana_msg::msg!("Hello, world!");
    0
}
