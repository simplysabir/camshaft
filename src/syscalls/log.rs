//! Logging, panic and abort. Reference: Agave `syscalls/src/logging.rs` and `SyscallAbort`,
//! `SyscallPanic` in `lib.rs`; log limit from `svm-log-collector`.

use super::read;
use crate::error::SyscallError;
use crate::interp::Vm;

/// Total bytes of log text kept before the log is cut off.
const LOG_LIMIT: usize = 10_000;

/// The program's log lines, as Agave records them.
#[derive(Debug, Default)]
pub struct Logs {
    pub lines: Vec<String>,
    bytes: usize,
    truncated: bool,
}

impl Logs {
    /// Adds a line unless it would reach the limit. The first line that would is replaced
    /// by `Log truncated`; later lines that still fit are kept.
    pub fn push(&mut self, line: String) {
        let bytes = self.bytes + line.len();
        if bytes < LOG_LIMIT {
            self.bytes = bytes;
            self.lines.push(line);
        } else if !self.truncated {
            self.truncated = true;
            self.lines.push("Log truncated".into());
        }
    }
}

pub(super) fn abort(_: &mut Vm, _: [u64; 5]) -> Result<u64, SyscallError> {
    Err(SyscallError::Abort)
}

pub(super) fn panic(
    vm: &mut Vm,
    [file, len, line, column, _]: [u64; 5],
) -> Result<u64, SyscallError> {
    let file = utf8(read(vm, file, len)?)?;
    Err(SyscallError::Panic(file.into(), line, column))
}

pub(super) fn log(vm: &mut Vm, [addr, len, ..]: [u64; 5]) -> Result<u64, SyscallError> {
    let line = format!("Program log: {}", utf8(read(vm, addr, len)?)?);
    vm.logs.push(line);
    Ok(0)
}

pub(super) fn log_64(vm: &mut Vm, [a, b, c, d, e]: [u64; 5]) -> Result<u64, SyscallError> {
    vm.logs.push(format!(
        "Program log: {a:#x}, {b:#x}, {c:#x}, {d:#x}, {e:#x}"
    ));
    Ok(0)
}

pub(super) fn log_pubkey(vm: &mut Vm, [addr, ..]: [u64; 5]) -> Result<u64, SyscallError> {
    let line = format!("Program log: {}", base58(vm.memory.slice(addr, 32)?));
    vm.logs.push(line);
    Ok(0)
}

pub(super) fn log_compute_units(vm: &mut Vm, _: [u64; 5]) -> Result<u64, SyscallError> {
    let line = format!("Program consumption: {} units remaining", vm.remaining());
    vm.logs.push(line);
    Ok(0)
}

fn utf8(bytes: &[u8]) -> Result<&str, SyscallError> {
    std::str::from_utf8(bytes).map_err(|e| SyscallError::InvalidString(e, bytes.to_vec()))
}

/// Bitcoin-alphabet base58, the way Solana prints public keys.
fn base58(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    // Digits in base 58, least significant first.
    let mut digits: Vec<u8> = Vec::new();
    for &byte in bytes {
        let mut carry = u32::from(byte);
        for digit in &mut digits {
            carry += u32::from(*digit) << 8;
            *digit = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    // Each leading zero byte is written as a '1'.
    let zeros = bytes.iter().take_while(|&&b| b == 0).count();
    let leading = std::iter::repeat_n('1', zeros);
    leading
        .chain(digits.iter().rev().map(|&d| ALPHABET[d as usize] as char))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base58_matches_known_keys() {
        assert_eq!(base58(&[0; 32]), "11111111111111111111111111111111");
        let mut token_program = [0u8; 32];
        token_program.copy_from_slice(&[
            6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180,
            133, 237, 95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
        ]);
        assert_eq!(
            base58(&token_program),
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        );
    }

    #[test]
    fn log_stops_at_the_limit() {
        let mut logs = Logs::default();
        logs.push("a".repeat(LOG_LIMIT - 1));
        logs.push("bb".into());
        logs.push("c".into());
        logs.push("".into());
        // The 9999-byte line fits. "bb" would reach 10001: truncated. "c" would reach 10000,
        // which is not below the limit either. The empty line still fits.
        assert_eq!(logs.lines[1..], ["Log truncated", ""]);
    }
}
