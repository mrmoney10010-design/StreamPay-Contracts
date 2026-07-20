# Ttl And Rent

This note documents the **ttl-and-rent** of the streampay-contract contract.

StreamPay uses Soroban's state expiration features to manage storage costs and reclaim abandoned data.

## Thresholds and Extensions

- **BUMP_THRESHOLD**: `100_000` ledgers (~6 days at 5 seconds per ledger). This acts as a floor. If an entry's TTL drops below this value, it will be extended.
- **BUMP_EXTEND**: `518_400` ledgers (~30 days at 5 seconds per ledger). The target TTL that each extension restores.

By using a `BUMP_THRESHOLD`, the contract avoids paying state rent (extension fees) on every single access while keeping actively used entries alive for about a month.

## Instance Storage
Instance storage holds configuration (admin, token addresses, stream counter). It is extended to `BUMP_EXTEND` every time:
- The contract is initialized.
- A new stream is created.
- An existing stream is topped up.
- An existing stream is extended.

## Persistent Storage
Persistent storage holds individual stream records keyed by ID. Each stream's TTL is bumped to `BUMP_EXTEND` on:
- Stream creation.
- Stream top ups.
- Stream extensions.
- Stream withdrawals.
- Stream cancellations.

Abandoned streams that are not accessed for longer than the extended TTL (~30 days) will eventually expire, and their storage can be reclaimed by the network, preventing state bloat.
