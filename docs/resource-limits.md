# Resource Limits

Soroban enforces hard per-transaction resource limits. Transactions that exceed
any limit are rejected before execution. This page documents the limits relevant
to StreamPay and explains how the contract stays well inside them.

## Network-wide limits

The values below reflect the Soroban protocol limits at the time of writing.
They are enforced by validators and can be adjusted through network governance.

| Resource | Per-transaction limit |
| --- | --- |
| CPU instructions | 100,000,000 |
| Memory bytes | 41,943,040 (40 MiB) |
| Ledger read entries | 40 |
| Ledger write entries | 25 |
| Read bytes (ledger) | 200,000 |
| Write bytes (ledger) | 66,560 |
| Transaction size | 71,680 bytes |
| Event bytes emitted | 8,192 |

## Instruction budget

Every WebAssembly opcode executed during a contract call is counted against the
instruction budget. Soroban's metering model charges for:

- WASM instruction execution.
- Host-function calls (e.g., storage reads/writes, cryptographic operations).
- Memory allocation and copying.

StreamPay is written in `#![no_std]` Rust compiled to wasm32. Its inner loops
(vesting arithmetic) consist of a handful of integer multiplications and
divisions, so the instruction footprint per call is low compared to the
100 million-instruction ceiling.

### Estimated instruction ranges

These are indicative ranges based on simulation on Testnet. Exact counts vary
by SDK version and network configuration.

| Entrypoint | Approximate instruction range |
| --- | --- |
| `initialize` | 500,000 – 1,000,000 |
| `create_stream` | 3,000,000 – 5,000,000 |
| `top_up` | 2,500,000 – 4,000,000 |
| `extend_stream` | 1,500,000 – 2,500,000 |
| `withdraw` | 2,500,000 – 4,000,000 |
| `cancel` | 3,000,000 – 6,000,000 |
| View functions | 500,000 – 1,500,000 |

All of these are well under the 100 million ceiling, leaving substantial
headroom even accounting for SDK version differences.

## Memory limits

The 40 MiB memory limit is the maximum WASM linear memory the contract may
allocate during a single invocation. StreamPay allocates memory only for:

- Decoded ledger entry values (a `Stream` struct is ~120 bytes).
- SDK-internal buffers for host-function calls.
- Stack frames for the call chain.

Typical StreamPay calls use well under 1 MiB of linear memory.

## Entry count limits

Soroban limits how many distinct ledger entries a single transaction may touch.
StreamPay's worst case is `create_stream`, which touches:

1. The contract instance entry (instance storage for Admin, Token, Counter).
2. The token SAC entry (one `transfer` call reads/writes the SAC's state).
3. The new `Stream(id)` persistent entry (written).

That is three entries, far below the 40-read / 25-write limits. Even a
hypothetical future batch call would have ample room.

## Write-byte budget

Each persistent `Stream` entry serializes to roughly 200–300 bytes (two
`Address` values, two `i128` amounts, two `u64` timestamps, and a `u32` status
tag). Counter and instance keys add another ~100 bytes. Total writes per
`create_stream` are well under the 66,560-byte limit.

## Keeping within limits

StreamPay was designed with resource efficiency in mind:

- **Single-entry reads**: every state-changing call reads at most one `Stream`
  entry and the instance storage blob.
- **No unbounded loops**: vesting arithmetic is O(1); there are no loops over
  storage.
- **`get_summary` batching**: clients should prefer `get_summary` over
  individual view getters to minimize round trips and total read bytes.
- **`#![no_std]`**: avoids pulling in the Rust standard library, keeping the
  compiled WASM binary small and instruction-count low.

## Checking resource usage

Always simulate before submitting to verify that your transaction fits within
the limits for the current network configuration:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source alice \
  --network testnet \
  --simulate-only \
  -- create_stream \
  --sender <SENDER> \
  --recipient <RECIPIENT> \
  --total_amount 1000000 \
  --start_time 1700000000 \
  --end_time 1702592000
```

See [gas-and-fees.md](gas-and-fees.md) for information on the Soroban fee model
and how to interpret simulation output.
