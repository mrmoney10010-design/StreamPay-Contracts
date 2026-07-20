# Gas and Fees

StreamPay runs on [Soroban](https://developers.stellar.org/docs/build/smart-contracts),
the smart-contract platform on Stellar. Soroban does not use a gas model;
instead it charges for three resource dimensions measured per transaction.

## Soroban fee model

Every Soroban transaction declares an explicit resource envelope and pays an
inclusion fee that covers both the network compute and the ledger storage it
consumes.

| Dimension | What it measures |
| --- | --- |
| **Instructions** | WebAssembly instructions executed during the call. |
| **Read bytes** | Bytes read from ledger entries (keys + values). |
| **Write bytes** | Bytes written to or created in ledger entries. |
| **Events** | Bytes emitted as contract events. |

Fees are denominated in stroops (1 XLM = 10,000,000 stroops). The exact fee
per unit of each dimension is set by network validators and can change through
governance. Always simulate before submitting.

## Simulating a transaction

The Stellar CLI `--simulate-only` flag runs the transaction against the network
state and prints the measured resources and the recommended fee:

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

The output includes:

- `instructions` — the measured instruction count.
- `readBytes` / `writeBytes` — storage I/O.
- `refundableFee` — the storage-rent component that is partially refundable.
- `fee` — total recommended inclusion fee in stroops.

Use the simulated values plus a small buffer (10–20 %) as your transaction's
resource limits when building transactions programmatically.

## Per-entrypoint cost guidance

### State-changing entrypoints

`create_stream` is the most expensive call because it performs a token
transfer, writes a new persistent ledger entry, increments the counter, bumps
instance storage, and emits a `created` event.

`top_up` and `extend_stream` are cheaper: they read and update one persistent
entry, bump instance storage, and emit one event. `top_up` also transfers a
token.

`withdraw` and `cancel` are similar in cost to `top_up`. `cancel` can perform
up to two token transfers (refund to sender and payment to recipient) so it is
the most instruction-heavy of the mutation calls after `create_stream`.

`initialize` is a one-time cost at deployment. It writes three instance keys
and bumps the TTL.

### View entrypoints

View functions (`get_stream`, `get_summary`, `streamed_amount`, etc.) only
read one persistent entry and perform arithmetic in WASM. They cost far less
than state-changing calls and are not subject to any token-transfer overhead.

`get_summary` is the most efficient way to fetch all headline figures: it reads
the stream entry once and derives every field in the same call, rather than
issuing five separate reads.

## Resource ceiling defaults

Soroban enforces network-wide per-transaction resource ceilings. At the time of
writing the relevant limits are:

| Resource | Limit |
| --- | --- |
| Instructions | 100,000,000 |
| Read bytes | 200,000 |
| Write bytes | 66,560 |
| Read entries | 40 |
| Write entries | 25 |

StreamPay's busiest call (`create_stream`) uses a small fraction of these
limits. See [resource-limits.md](resource-limits.md) for a fuller discussion of
the instruction budget and how StreamPay stays well within it.

## Storage rent

Storage fees are split into an **inclusion fee** (paid upfront) and a
**rent** component that keeps ledger entries alive over time. Entries have a
TTL measured in ledgers; once the TTL hits zero the entry is evictable.
StreamPay proactively bumps TTLs on every write so active streams never expire
accidentally. See [ttl-and-rent.md](ttl-and-rent.md) for the bump constants and
strategy.
