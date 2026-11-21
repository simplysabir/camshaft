//! Syscalls, checked against the behavior in Agave v3.0.8's source (`syscalls/src`).
//! The reference VM has none of Agave's syscalls, so these are not differential.

use camshaft::error::{AccessType, SyscallError, VmError};
use camshaft::executable::{Executable, hash_symbol_name};
use camshaft::interp::Vm;
use camshaft::memory::{HEAP_START, PROGRAM_START, STACK_START};
use std::process::Command;

type Slot = [u8; 8];

fn slot(opc: u8, dst: u8, src: u8, off: i16, imm: i32) -> Slot {
    let [o0, o1] = off.to_le_bytes();
    let [i0, i1, i2, i3] = imm.to_le_bytes();
    [opc, src << 4 | dst, o0, o1, i0, i1, i2, i3]
}

fn mov(dst: u8, imm: i32) -> Slot {
    slot(0xb7, dst, 0, 0, imm)
}

fn lddw(dst: u8, value: u64) -> [Slot; 2] {
    [
        slot(0x18, dst, 0, 0, value as i32),
        slot(0, 0, 0, 0, (value >> 32) as i32),
    ]
}

fn call(name: &str) -> Slot {
    slot(0x85, 0, 0, 0, hash_symbol_name(name.as_bytes()) as i32)
}

/// ldxw r0, [r4+0]
const LOAD_R4: Slot = [0x61, 0x40, 0, 0, 0, 0, 0, 0];
const EXIT: Slot = [0x95, 0, 0, 0, 0, 0, 0, 0];
/// The last 8 bytes of the first stack frame, just below r10.
const SCRATCH: u64 = STACK_START + 4096 - 8;

/// Code followed by read-only data, like `.text` and `.rodata`. `code` gets the data's address.
fn with_data(data: &[u8], code: impl Fn(u64) -> Vec<Slot>) -> Executable {
    let data_addr = PROGRAM_START + 8 * code(0).len() as u64;
    let text = code(data_addr).as_flattened().to_vec();
    let mut ro = text.clone();
    ro.extend(data);
    Executable {
        ro,
        ro_addr: PROGRAM_START,
        text: 0..text.len(),
        entry_pc: 0,
        functions: Default::default(),
        syscall_names: Default::default(),
    }
}

fn code_only(code: &[Slot]) -> Executable {
    with_data(&[], |_| code.to_vec())
}

/// Runs and returns the result, the log lines and the final pc.
fn run(exe: &Executable) -> (Result<u64, VmError>, Vec<String>, usize) {
    let mut vm = Vm::new(exe);
    let result = vm.run(1000);
    (result, vm.logs.lines, vm.pc)
}

fn syscall_error(e: SyscallError) -> Result<u64, VmError> {
    Err(VmError::SyscallError(Box::new(e)))
}

#[test]
fn hello_world_built_with_cargo_build_sbf() {
    let exe = Executable::from_elf(include_bytes!("programs/hello.so")).unwrap();
    exe.verify().unwrap();
    let (result, logs, _) = run(&exe);
    assert_eq!(result, Ok(0));
    assert_eq!(logs, ["Program log: Hello, world!"]);
}

#[test]
fn cli_runs_hello_and_names_unknown_syscalls() {
    let camshaft = env!("CARGO_BIN_EXE_camshaft");
    let out = Command::new(camshaft)
        .args(["run", "tests/programs/hello.so"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.starts_with("Program log: Hello, world!\n"),
        "{stdout}"
    );

    // This test ELF calls a syscall named just `log`, which Solana does not have.
    let fixture = "tests/programs/syscall_reloc_64_32_sbpfv0.so";
    let out = Command::new(camshaft)
        .args(["run", fixture])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("unknown syscall `log`")
    );
}

/// Logs `data` with `sol_log_`.
fn log_bytes(data: &[u8]) -> Executable {
    with_data(data, |addr| {
        [
            &lddw(1, addr)[..],
            &[mov(2, data.len() as i32), call("sol_log_"), EXIT],
        ]
        .concat()
    })
}

#[test]
fn sol_log_writes_utf8_text() {
    assert_eq!(run(&log_bytes(b"hi")).1, ["Program log: hi"]);
}

#[test]
fn sol_log_rejects_bad_text_and_bad_pointers() {
    let (result, _, pc) = run(&log_bytes(&[0xff]));
    let invalid_utf8 = String::from_utf8(vec![0xff]).unwrap_err().utf8_error();
    assert_eq!(
        result,
        syscall_error(SyscallError::InvalidString(invalid_utf8, vec![0xff]))
    );
    assert_eq!(pc, 3, "pc stays on the call");

    // A pointer into unmapped memory fails with the access violation inside.
    let unmapped = code_only(&[mov(1, 0), mov(2, 1), call("sol_log_"), EXIT]);
    let violation = VmError::AccessViolation(AccessType::Load, 0, 1, "unallocated");
    assert_eq!(
        run(&unmapped).0,
        syscall_error(SyscallError::Memory(violation))
    );

    // Zero length is fine whatever the pointer.
    let empty = code_only(&[mov(1, 0), mov(2, 0), call("sol_log_"), EXIT]);
    assert_eq!(run(&empty).1, ["Program log: "]);
}

#[test]
fn sol_log_64_pubkey_and_compute_units() {
    let log_64 = code_only(&[
        mov(1, 1),
        mov(2, 2),
        mov(3, 3),
        mov(4, 4),
        mov(5, -1),
        call("sol_log_64_"),
        EXIT,
    ]);
    assert_eq!(
        run(&log_64).1,
        ["Program log: 0x1, 0x2, 0x3, 0x4, 0xffffffffffffffff"]
    );

    let pubkey = with_data(&[0; 32], |addr| {
        [&lddw(1, addr)[..], &[call("sol_log_pubkey"), EXIT]].concat()
    });
    assert_eq!(
        run(&pubkey).1,
        ["Program log: 11111111111111111111111111111111"]
    );

    // The limit is 1000 and the call is the first instruction.
    let units = code_only(&[call("sol_log_compute_units_"), EXIT]);
    assert_eq!(run(&units).1, ["Program consumption: 999 units remaining"]);
}

#[test]
fn abort_and_panic_stop_the_program() {
    assert_eq!(
        run(&code_only(&[call("abort"), EXIT])).0,
        syscall_error(SyscallError::Abort)
    );

    let panic = with_data(b"src/lib.rs", |addr| {
        [
            &lddw(1, addr)[..],
            &[mov(2, 10), mov(3, 7), mov(4, 9), call("sol_panic_"), EXIT],
        ]
        .concat()
    });
    assert_eq!(
        run(&panic).0,
        syscall_error(SyscallError::Panic("src/lib.rs".into(), 7, 9))
    );
}

/// Copies `n` bytes from `data[offset..]` to `dst` with syscall `name`, then loads the 4 bytes
/// at the scratch slot into r0.
fn copy(name: &str, dst: u64, offset: u64, n: i32) -> Result<u64, VmError> {
    let exe = with_data(&[1, 2, 3, 4, 5, 6, 7, 8], |data| {
        [
            &lddw(1, dst)[..],
            &lddw(2, data + offset),
            &[mov(3, n), call(name)],
            &lddw(4, SCRATCH),
            &[LOAD_R4, EXIT],
        ]
        .concat()
    });
    run(&exe).0
}

#[test]
fn memcpy_and_memmove() {
    assert_eq!(copy("sol_memcpy_", SCRATCH, 0, 4), Ok(0x0403_0201));
    assert_eq!(copy("sol_memmove_", SCRATCH, 2, 4), Ok(0x0605_0403));
    // Zero bytes is fine even to an unmapped address.
    assert_eq!(copy("sol_memcpy_", 0, 0, 0), Ok(0));
    // With both pointers bad, the destination is reported, as Agave checks it first.
    let violation = VmError::AccessViolation(AccessType::Store, 0, 4, "unallocated");
    assert_eq!(
        copy("sol_memmove_", 0, 1 << 20, 4),
        syscall_error(SyscallError::Memory(violation))
    );
    // The program region is read-only.
    let violation = VmError::AccessViolation(AccessType::Store, PROGRAM_START, 4, "program");
    assert_eq!(
        copy("sol_memcpy_", PROGRAM_START, 0, 4),
        syscall_error(SyscallError::Memory(violation))
    );
}

#[test]
fn memcpy_rejects_overlap_but_memmove_allows_it() {
    // Copy the scratch bytes onto themselves, shifted by one.
    let shift = |name| {
        code_only(
            &[
                &lddw(1, SCRATCH + 1)[..],
                &lddw(2, SCRATCH),
                &[mov(3, 4), call(name), EXIT],
            ]
            .concat(),
        )
    };
    assert_eq!(
        run(&shift("sol_memcpy_")).0,
        syscall_error(SyscallError::CopyOverlapping)
    );
    assert_eq!(run(&shift("sol_memmove_")).0, Ok(0));
}

#[test]
fn memset_fills_bytes() {
    // ldxdw r0, [r10-8]
    let load = slot(0x79, 0, 10, -8, 0);
    let exe = code_only(
        &[
            &lddw(1, SCRATCH)[..],
            &[mov(2, 0x1ab), mov(3, 8), call("sol_memset_"), load, EXIT],
        ]
        .concat(),
    );
    // Only the low byte of the value is used.
    assert_eq!(run(&exe).0, Ok(0xabab_abab_abab_abab));
}

/// Compares `a` with `b` and writes the result to `result`, then loads it into r0.
fn memcmp(a: &[u8], b: &[u8], result: u64) -> Result<u64, VmError> {
    let exe = with_data(&[a, b].concat(), |data| {
        let b_addr = data + a.len() as u64;
        [
            &lddw(1, data)[..],
            &lddw(2, b_addr),
            &[mov(3, a.len() as i32)],
            &lddw(4, result),
            &[call("sol_memcmp_"), LOAD_R4, EXIT],
        ]
        .concat()
    });
    run(&exe).0
}

#[test]
fn memcmp_writes_the_first_difference() {
    assert_eq!(memcmp(b"abc", b"abc", SCRATCH), Ok(0));
    assert_eq!(memcmp(b"abc", b"abd", SCRATCH), Ok((-1i32) as u32 as u64));
    assert_eq!(memcmp(b"b", b"a", SCRATCH), Ok(1));
    assert_eq!(
        memcmp(b"abc", b"abc", SCRATCH + 1),
        syscall_error(SyscallError::UnalignedPointer)
    );
    let violation = VmError::AccessViolation(AccessType::Store, 0, 4, "unallocated");
    assert_eq!(
        memcmp(b"a", b"a", 0),
        syscall_error(SyscallError::Memory(violation))
    );
}

#[test]
fn alloc_free_bumps_through_the_heap() {
    let alloc = |size: i32| [mov(1, size), mov(2, 0), call("sol_alloc_free_")];
    let exe = code_only(&[&alloc(3)[..], &alloc(3), &[EXIT]].concat());
    let mut vm = Vm::new(&exe);
    // The second allocation starts at the next 8-byte boundary.
    assert_eq!(vm.run(1000), Ok(HEAP_START + 8));
    assert_eq!(vm.heap_used, 11);

    // Too big returns 0, and so does a free.
    assert_eq!(
        run(&code_only(&[&alloc(32 * 1024 + 1)[..], &[EXIT]].concat())).0,
        Ok(0)
    );
    let free = code_only(&[mov(1, 8), mov(2, 0x10), call("sol_alloc_free_"), EXIT]);
    assert_eq!(run(&free).0, Ok(0));
}
