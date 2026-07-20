# TTL and Rent

Soroban ledger entries are not permanent by default. Each entry carries a
**time-to-live (TTL)** measured in ledgers. When the TTL reaches zero the entry
becomes *evictable*: the network may remove it to reclaim storage, after which
it cannot be read until it is explicitly restored.

StreamPay manages TTLs proactively so that active streams are never
accidentally evicted.

## TTL constants

Both constants are defined in [`src/storage.rs`](../src/storage.rs) and apply
equally to instance storage and every persistent `Stream` entry.

| Constant | Value | Approx. real time (5 s/ledger) |
| --- | --- | --- |
| `BUMP_THRESHOLD` | `100_000` ledgers | ~6 days |
| `BUMP_EXTEND` | `518_400` ledgers | ~30 days |

### BUMP_THRESHOLD

Before extending a TTL, StreamPay checks whether the remaining TTL is already
above `BUMP_THRESHOLD`. If it is, the extension is skipped. This avoids paying
the rent fee on every single read when the entry is healthy.

### BUMP_EXTEND

When an extension is needed, the TTL is set to `BUMP_EXTEND` ledgers into the
future. This guarantees that any entry touched at least once every 30 days will
never expire.

## When bumps happen

| Operation | Storage bumped |
| --- | --- |
| `initialize` | Instance (once, at setup). |
| `create_stream` | Instance + new `Stream(id)` entry. |
| `top_up` | Instance + existing `Stream(id)` entry. |
| `extend_stream` | Instance + existing `Stream(id)` entry. |
| `withdraw` | `Stream(id)` entry only (no instance write). |
| `cancel` | `Stream(id)` entry only (no instance write). |
| View functions | No bump (read-only). |

Read-only calls do not pay any TTL extension fee. The bump is conditional on
the remaining TTL being below `BUMP_THRESHOLD`, so callers do not pay the fee
on every write either if the entry was recently bumped.

## Rent cost implications

Soroban's rent model charges a fee proportional to:

- The **size** of the entry (in bytes).
- The **number of ledgers** by which the TTL is extended.

A `Stream` entry is approximately 200–300 bytes. Extending by `BUMP_EXTEND`
(518,400 ledgers) from zero costs a small number of stroops at current network
rates. In practice this cost is included in the transaction fee estimated by
the simulation step and is negligible compared to the token amounts being
streamed.

## Abandoned streams

Streams that are no longer actively used will stop receiving TTL bumps once
their sender and recipient stop interacting with them. After the TTL reaches
zero the network may evict the entry. At that point:

- The stream data is no longer readable via `get_stream`.
- Any escrowed balance effectively becomes inaccessible (there is no
  built-in recovery path in the current contract version).

**Best practice**: call `withdraw` or `cancel` before walking away from a
stream. This transfers the funds to the appropriate parties and leaves the
entry in a terminal state that can safely expire.

## Restoring an evicted entry

Soroban provides a `RestoreFootprint` operation that can restore an evicted
persistent entry from the network's historical archive. If a stream entry is
evicted before the funds are recovered, a restore operation can bring it back,
after which a `withdraw` or `cancel` call can claim the balance. This is an
advanced recovery path; see the Soroban documentation for details.

## Instance storage and the contract lifecycle

The contract's instance entry (which holds `Admin`, `Token`, and `Counter`)
shares a single TTL. Because every write call bumps the instance TTL, the
contract as a whole stays alive as long as any user is creating or modifying
streams. If the contract is completely abandoned, the instance entry will
eventually expire, making the contract uninvokeable until a restore is
performed.

## Further reading

- [gas-and-fees.md](gas-and-fees.md) — fee model overview and simulation
  guidance.
- [resource-limits.md](resource-limits.md) — instruction budget and entry
  count limits.
- [Soroban state archival docs](https://developers.stellar.org/docs/build/smart-contracts/state-archival)
  — authoritative reference for TTL, rent, and restore operations.
