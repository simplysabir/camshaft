//! What a running program can see and change besides its memory: its id, sysvars, logs,
//! heap, return data and compute meter. Agave keeps these in its `InvokeContext`.

use crate::error::SyscallError;
use serde::Deserialize;

pub type Pubkey = [u8; 32];

/// Total bytes of log text kept before the log is cut off.
const LOG_LIMIT: usize = 10_000;

#[derive(Debug, Default)]
pub struct Context {
    pub program_id: Pubkey,
    pub clock: Clock,
    pub rent: Rent,
    pub logs: Logs,
    /// Bytes of the heap handed out by `sol_alloc_free_`.
    pub heap_used: u64,
    /// Set by `sol_set_return_data`, read by the caller.
    pub return_data: Vec<u8>,
    /// Compute units left. Each instruction costs 1; syscalls cost more.
    pub remaining: u64,
}

impl Context {
    /// Charges `units`. Running out leaves the meter at 0 and fails.
    pub fn consume(&mut self, units: u64) -> Result<(), SyscallError> {
        let exceeded = self.remaining < units;
        self.remaining = self.remaining.saturating_sub(units);
        if exceeded {
            Err(SyscallError::ComputationalBudgetExceeded)
        } else {
            Ok(())
        }
    }
}

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

/// The clock sysvar, laid out as programs read it (40 bytes).
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Clock {
    pub slot: u64,
    pub epoch_start_timestamp: i64,
    pub epoch: u64,
    pub leader_schedule_epoch: u64,
    pub unix_timestamp: i64,
}

impl Clock {
    pub fn to_bytes(&self) -> [u8; 40] {
        let fields = [
            self.slot,
            self.epoch_start_timestamp as u64,
            self.epoch,
            self.leader_schedule_epoch,
            self.unix_timestamp as u64,
        ];
        let mut bytes = [0; 40];
        for (chunk, field) in bytes.chunks_mut(8).zip(fields) {
            chunk.copy_from_slice(&field.to_le_bytes());
        }
        bytes
    }
}

/// The rent sysvar, laid out as programs read it (24 bytes, the last 7 are padding).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rent {
    pub lamports_per_byte_year: u64,
    pub exemption_threshold: f64,
    pub burn_percent: u8,
}

/// Mainnet's rent sysvar as of 2026-11-21. It is cluster state and can change; fixtures can
/// set their own.
impl Default for Rent {
    fn default() -> Self {
        Self {
            lamports_per_byte_year: 5080,
            exemption_threshold: 1.0,
            burn_percent: 50,
        }
    }
}

impl Rent {
    pub fn to_bytes(&self) -> [u8; 24] {
        let mut bytes = [0; 24];
        bytes[..8].copy_from_slice(&self.lamports_per_byte_year.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.exemption_threshold.to_le_bytes());
        bytes[16] = self.burn_percent;
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn running_out_of_units() {
        let mut ctx = Context {
            remaining: 10,
            ..Context::default()
        };
        assert_eq!(ctx.consume(4), Ok(()));
        assert_eq!(
            ctx.consume(7),
            Err(SyscallError::ComputationalBudgetExceeded)
        );
        assert_eq!(ctx.remaining, 0);
    }
}
