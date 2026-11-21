# 06: Program input and compute units

camshaft can now run a real Solana instruction: accounts go in, the program runs with a
compute budget, and the changed accounts come out, checked against the runtime's rules.

```
$ camshaft run tests/programs/counter.so --input tests/programs/counter.json
Program 29d2S7vB453rNYFdR5Ycwt7y9haRT5fwVwL9zTmBhfV2 invoke [1]
Program log: count: 7
Program 29d2S7vB453rNYFdR5Ycwt7y9haRT5fwVwL9zTmBhfV2 consumed 773 of 200000 compute units
Program 29d2S7vB453rNYFdR5Ycwt7y9haRT5fwVwL9zTmBhfV2 success

Compute units: 773
Instructions: 643
Changed 2DYKaRPBeNM5WdW8rNsYEktjPrnd89Mm4Lzp3qonSzoj:
  data: 0200000000000000 -> 0700000000000000
```

## Which runtime this matches

**Agave 4.4.1**, which ran about 81% of mainnet stake when this was written. Its program
runtime changed a lot since 3.0, so this is the version camshaft copies.

## The fixture file

One JSON file describes one instruction:

```json
{
  "program_id": "29d2S7vB453rNYFdR5Ycwt7y9haRT5fwVwL9zTmBhfV2",
  "instruction_data": "0500000000000000",
  "accounts": [
    { "pubkey": "2DYK...", "is_writable": true, "owner": "29d2...", "lamports": 1000000, "data": "0200000000000000" }
  ]
}
```

- Keys are base58, data is hex.
- `accounts` is both the instruction's account list and each account's state. List a key
  twice to pass it twice.
- Optional: `is_signer`, `executable`, `compute_unit_limit` (default 200,000), `clock`, `rent`.
- Missing account fields default to 0, empty, false, and the system program as owner.

## The input region

The program gets everything in one buffer at `0x4_0000_0000`. r1 points at the start, and r2
points at the instruction data:

```
u64 number of accounts
each account:  0xff, is_signer, is_writable, executable, 4 zero bytes,
               key (32), owner (32), lamports (u64), data length (u64),
               data, then 10 KiB of zeros for growth, padded to 8 bytes,
               rent epoch (always u64::MAX)
  or a repeat: index of its first mention, 7 zero bytes
u64 instruction data length, instruction data, program id (32)
```

The 10 KiB after each account's data is how a program can grow an account by up to 10 KiB.

## After the run

camshaft reads the accounts back and checks every change, the way the runtime does:

| change | allowed only if | otherwise |
|---|---|---|
| lamports go down | the program owns the account | `ExternalAccountLamportSpend` |
| lamports change | the account is writable | `ReadonlyLamportChange` |
| data grows more than 10 KiB | never | `InvalidRealloc` |
| data or its length changes | writable and owned | `ReadonlyDataModified` / `ExternalAccountDataModified` |
| owner changes | writable, owned, data all zeros | `ModifiedProgramId` |
| total lamports change | never | `UnbalancedInstruction` |

If the instruction fails, no account changes are kept.

## New syscalls

| name | what it does |
|---|---|
| `sol_sha256`, `sol_keccak256` | hash a list of byte slices into 32 bytes |
| `sol_create_program_address` | sha256 of seeds + program id + `"ProgramDerivedAddress"`; returns 1 if that lands on the ed25519 curve |
| `sol_try_find_program_address` | tries bump seeds 255 down to 1 until one is off the curve |
| `sol_get_clock_sysvar`, `sol_get_rent_sysvar` | copy the clock (40 bytes) or rent (24 bytes) into memory below the input region |
| `sol_set_return_data`, `sol_get_return_data` | up to 1024 bytes a program hands back to its caller |

A program derived address (PDA) must be off the curve so no private key can sign for it.
The curve check uses `curve25519-dalek`; the hashes use `sha2` and `sha3`.

## Compute units

Every instruction costs 1 unit. Syscalls cost more, charged before they do any work:

| syscall | cost |
|---|---|
| `sol_log_` | 100, or the message length if longer |
| `sol_log_64_`, `sol_log_pubkey`, `sol_log_compute_units_` | 100 |
| memory syscalls | 10, or 1 per 250 bytes if more |
| `sol_sha256`, `sol_keccak256` | 85, plus per slice: 10 or half its length, whichever is more |
| `sol_create_program_address` | 1,500 |
| `sol_try_find_program_address` | 1,500, plus 1,500 for every bump that fails |
| `sol_get_clock_sysvar`, `sol_get_rent_sysvar` | 100 plus the struct size (40 or 24) |
| `sol_set_return_data` | 100 plus 1 per 250 bytes |
| `sol_get_return_data` | 100, plus (length + 32) / 250 |

Running out inside a syscall fails with `Computational budget exceeded`. A crash outside a
syscall (like an access violation) charges the whole budget.

## How a run ends

| program does | result |
|---|---|
| returns 0 | success |
| returns a non-zero code | that code as an error, like `custom program error: 0x2a` |
| a syscall fails | `Program failed to complete`, the log says why |
| runs out of units | `Computational budget exceeded` |
| crashes | `Program failed to complete` |

## How we know it is right

`tests/runtime.rs` runs the same instruction through camshaft and through Agave 4.4.1
(using mollusk) and compares the result, every log line, compute units, final accounts and
return data. Agave runs with the feature flags mainnet had on 2026-11-21. It covers:

- `counter` (`tests/programs/counter/`), plus three ways to make it fail.
- SPL Memo, dumped from mainnet with `solana program dump`, plus a missing signature and bad UTF-8.
- `tour` (`tests/programs/tour/`): one scenario that uses every new syscall, one that logs r2,
  and twelve that each fail differently (panic, custom error, read-only write, unbalanced
  lamports, bad pointer, endless loop, too many PDA tries, log overflow, illegal owner change,
  too many hash slices, shrinking an account it does not own, a sysvar written into the input).
