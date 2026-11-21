//! Virtual memory: four regions with bounds and write checks.
//! Reference: `memory_region.rs` in anza-xyz/sbpf.

use crate::error::{AccessType, VmError};
use crate::insn::Size;
use crate::version::Version;
use std::ops::Range;

/// v3 maps its read-only data here. v0 maps nothing here.
pub const RODATA_START: u64 = 0;
pub const PROGRAM_START: u64 = 0x1_0000_0000;
pub const STACK_START: u64 = 0x2_0000_0000;
pub const HEAP_START: u64 = 0x3_0000_0000;
pub const INPUT_START: u64 = 0x4_0000_0000;

pub const STACK_FRAME_SIZE: u64 = 4096;
pub const MAX_CALL_DEPTH: u64 = 64;
pub const HEAP_SIZE: usize = 32 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Region {
    Program,
    Stack,
    Heap,
    Input,
}

pub struct Memory<'a> {
    /// Read-only: the executable's read-only data, plus its code in v0.
    pub program: &'a [u8],
    /// Where `program` starts: in region 1 (v0) or region 0 (v3).
    pub program_addr: u64,
    /// One frame per call level. In v0, each frame is followed by an unmapped gap in
    /// virtual addresses.
    pub stack: Vec<u8>,
    pub heap: Vec<u8>,
    pub input: Vec<u8>,
    /// When `Some`, each successful write's address and length, for the tracer.
    pub writes: Option<Vec<(u64, usize)>>,
    stack_gaps: bool,
}

impl<'a> Memory<'a> {
    pub fn new(program: &'a [u8], program_addr: u64, version: Version) -> Self {
        Self {
            program,
            program_addr,
            stack: vec![0; (STACK_FRAME_SIZE * MAX_CALL_DEPTH) as usize],
            heap: vec![0; HEAP_SIZE],
            input: Vec::new(),
            writes: None,
            stack_gaps: version.stack_frame_gaps(),
        }
    }

    /// Reads a little-endian number of the given size.
    pub fn load(&self, addr: u64, size: Size) -> Result<u64, VmError> {
        let len = size.bytes();
        let mut bytes = [0; 8];
        bytes[..len].copy_from_slice(self.slice(addr, len)?);
        Ok(u64::from_le_bytes(bytes))
    }

    /// Writes the low bytes of `value`, little-endian.
    pub fn store(&mut self, addr: u64, size: Size, value: u64) -> Result<(), VmError> {
        let len = size.bytes();
        self.slice_mut(addr, len)?
            .copy_from_slice(&value.to_le_bytes()[..len]);
        Ok(())
    }

    /// `len` bytes at `addr`, for reading.
    pub fn slice(&self, addr: u64, len: usize) -> Result<&[u8], VmError> {
        let (region, range) = self
            .locate(addr, len)
            .ok_or_else(|| self.violation(AccessType::Load, addr, len))?;
        Ok(&self.bytes(region)[range])
    }

    /// `len` bytes at `addr`, for writing. Fails for the read-only program region.
    pub fn slice_mut(&mut self, addr: u64, len: usize) -> Result<&mut [u8], VmError> {
        match self.locate(addr, len) {
            Some((region, range)) if region != Region::Program => {
                if let Some(writes) = &mut self.writes {
                    writes.push((addr, len));
                }
                Ok(&mut self.bytes_mut(region)[range])
            }
            _ => Err(self.violation(AccessType::Store, addr, len)),
        }
    }

    /// True if two accesses touch the same underlying bytes. Agave compares host addresses,
    /// so two accesses in different stack frames can overlap through the gap rule.
    pub fn overlaps(&self, (a, a_len): (u64, usize), (b, b_len): (u64, usize)) -> bool {
        match (self.locate(a, a_len), self.locate(b, b_len)) {
            (Some((ra, a)), Some((rb, b))) => ra == rb && a.start < b.end && b.start < a.end,
            _ => false,
        }
    }

    /// Finds the region and byte range for an access, or `None` if any byte is unmapped.
    fn locate(&self, addr: u64, len: usize) -> Option<(Region, Range<usize>)> {
        let (region, mut offset) = self.offset_in(addr)?;
        if region == Region::Stack && self.stack_gaps {
            // Only the start address is checked against the gaps, as in the reference.
            if (offset / STACK_FRAME_SIZE) % 2 == 1 {
                return None;
            }
            offset = offset / (2 * STACK_FRAME_SIZE) * STACK_FRAME_SIZE + offset % STACK_FRAME_SIZE;
        }
        let start = usize::try_from(offset).ok()?;
        let end = start.checked_add(len)?;
        (end <= self.bytes(region).len()).then_some((region, start..end))
    }

    fn violation(&self, access: AccessType, addr: u64, len: usize) -> VmError {
        let len = len as u64;
        let frame = (addr as i64).saturating_sub(STACK_START as i64) / STACK_FRAME_SIZE as i64;
        if (-1..=MAX_CALL_DEPTH as i64).contains(&frame) {
            return VmError::StackAccessViolation(access, addr, len, frame);
        }
        // Named by the address's region number, not by what is mapped there: v3's read-only
        // data in region 0 has no name.
        let name = match self.offset_in(addr) {
            Some((region, offset)) if offset < self.virtual_len(region) => {
                match addr & !0xffff_ffff {
                    PROGRAM_START => "program",
                    STACK_START => "stack",
                    HEAP_START => "heap",
                    INPUT_START => "input",
                    _ => "allocated",
                }
            }
            _ => "unallocated",
        };
        VmError::AccessViolation(access, addr, len, name)
    }

    /// The region an address falls in and its offset from the region's start.
    fn offset_in(&self, addr: u64) -> Option<(Region, u64)> {
        let region = match addr & !0xffff_ffff {
            STACK_START => Region::Stack,
            HEAP_START => Region::Heap,
            INPUT_START => Region::Input,
            // The top 32 bits pick the region.
            high if high == self.program_addr & !0xffff_ffff => Region::Program,
            _ => return None,
        };
        let start = match region {
            Region::Program => self.program_addr,
            Region::Stack => STACK_START,
            Region::Heap => HEAP_START,
            Region::Input => INPUT_START,
        };
        Some((region, addr.checked_sub(start)?))
    }

    fn bytes(&self, region: Region) -> &[u8] {
        match region {
            Region::Program => self.program,
            Region::Stack => &self.stack,
            Region::Heap => &self.heap,
            Region::Input => &self.input,
        }
    }

    fn bytes_mut(&mut self, region: Region) -> &mut [u8] {
        match region {
            Region::Program => unreachable!("the program region is read-only"),
            Region::Stack => &mut self.stack,
            Region::Heap => &mut self.heap,
            Region::Input => &mut self.input,
        }
    }

    /// Size in virtual addresses. A v0 stack counts its gaps.
    fn virtual_len(&self, region: Region) -> u64 {
        let len = self.bytes(region).len() as u64;
        if region == Region::Stack && self.stack_gaps {
            2 * len
        } else {
            len
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Size::{B, DW, H, W};

    #[test]
    fn stack_frames_have_gaps() {
        let mut memory = Memory::new(&[], PROGRAM_START, Version::V0);
        memory.store(STACK_START + 8, DW, 0x1122).unwrap();
        // Frame 1 starts at 2 * 4096 in virtual space, right after frame 0's host bytes.
        memory
            .store(STACK_START + 2 * STACK_FRAME_SIZE, B, 0xab)
            .unwrap();
        assert_eq!(memory.stack[STACK_FRAME_SIZE as usize], 0xab);
        assert_eq!(memory.load(STACK_START + 8, H), Ok(0x1122));
        assert_eq!(
            memory.load(STACK_START + STACK_FRAME_SIZE, B),
            Err(VmError::StackAccessViolation(
                AccessType::Load,
                STACK_START + STACK_FRAME_SIZE,
                1,
                1
            ))
        );
    }

    #[test]
    fn program_is_read_only() {
        let mut memory = Memory::new(&[1, 2, 3, 4], PROGRAM_START, Version::V0);
        assert_eq!(memory.load(PROGRAM_START + 1, H), Ok(0x0302));
        assert_eq!(
            memory.store(PROGRAM_START, B, 0),
            Err(VmError::AccessViolation(
                AccessType::Store,
                PROGRAM_START,
                1,
                "program"
            ))
        );
        assert_eq!(
            memory.load(PROGRAM_START + 2, W),
            Err(VmError::AccessViolation(
                AccessType::Load,
                PROGRAM_START + 2,
                4,
                "program"
            ))
        );
    }

    #[test]
    fn v3_has_no_stack_gaps_and_data_at_zero() {
        let mut memory = Memory::new(&[1, 2, 3, 4], 0, Version::V3);
        memory
            .store(STACK_START + STACK_FRAME_SIZE, B, 0xab)
            .unwrap();
        assert_eq!(memory.stack[STACK_FRAME_SIZE as usize], 0xab);
        assert_eq!(memory.load(1, B), Ok(2));
        assert_eq!(
            memory.store(0, B, 0),
            Err(VmError::AccessViolation(
                AccessType::Store,
                0,
                1,
                "allocated"
            ))
        );
        // The code is not mapped as data in v3.
        assert_eq!(
            memory.load(PROGRAM_START, B),
            Err(VmError::AccessViolation(
                AccessType::Load,
                PROGRAM_START,
                1,
                "unallocated"
            ))
        );
    }

    #[test]
    fn unmapped_addresses_are_unallocated() {
        let memory = Memory::new(&[], PROGRAM_START, Version::V0);
        for addr in [0, 8, INPUT_START, 0x5_0000_0000, u64::MAX] {
            assert_eq!(
                memory.load(addr, B),
                Err(VmError::AccessViolation(
                    AccessType::Load,
                    addr,
                    1,
                    "unallocated"
                ))
            );
        }
    }
}
