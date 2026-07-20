# Entrypoint Reference

This document describes all public entrypoints of the `streampay-contract`.

## Initialization

### `initialize(admin, token) → Result<(), Error>`

One-shot setup. Stores the admin address, the SAC token address, initializes
the stream counter to `0`, sets `total_supply` to `0`, and seeds the
`supply_cap` to `i128::MAX` (effectively unlimited). Returns
`AlreadyInitialized` if called more than once.

## Admin operations

### `set_supply_cap(new_cap) → Result<(), Error>`

Admin-only. Updates the global cap on the total amount of tokens that may be
simultaneously held in escrow. `new_cap` must be positive. Setting the cap
below the current `total_supply` does not affect existing streams but blocks
new `create_stream` and `top_up` calls until supply drops below the new cap.
Emits a `capadmin` event. Returns `NotInitialized` if called before
`initialize`.

## Stream lifecycle

### `create_stream(sender, recipient, total_amount, start_time, end_time) → Result<u64, Error>`

Creates a new stream and escrows `total_amount` from `sender`. Requires
`sender` authorization. Returns the new stream's monotonic id. Fails with:

- `NotInitialized` — contract not yet initialized
- `InvalidAmount` / `AmountBelowMinimum` — `total_amount` is non-positive or `< MIN_STREAM_AMOUNT`
- `InvalidTimeRange` — `end_time <= start_time`
- `EndTimeInPast` — `end_time <= ledger.timestamp()`
- `SupplyCapExceeded` — `total_supply + total_amount > supply_cap`
- `Overflow` — stream counter would overflow

### `top_up(id, sender, amount) → Result<i128, Error>`

Adds `amount` to an active stream's total escrow and increases the per-second
vesting rate. Only the original sender may call this. Returns the new `total`.
Fails with `StreamNotActive` for cancelled or completed streams,
`SupplyCapExceeded` if the cap would be breached.

### `extend_stream(id, sender, new_end) → Result<(), Error>`

Pushes the stream's end time to `new_end`, slowing the vesting rate. Only the
original sender may call this; `new_end` must be strictly later than the
current end.

### `withdraw(id, recipient) → Result<i128, Error>`

Transfers the vested-but-unwithdrawn balance to `recipient`. Requires
`recipient` authorization. Decrements `total_supply` by the amount
transferred. Returns the amount paid.

### `cancel(id, caller) → Result<(), Error>`

Cancels an active stream. The `caller` must be the sender or recipient.
Distributes the vested-but-unwithdrawn portion to the recipient and the
unvested remainder back to the sender. Decrements `total_supply` by the sum
of both payouts.

## View entrypoints

| Entrypoint | Returns | Description |
|---|---|---|
| `get_admin()` | `Address` | The admin address |
| `get_token()` | `Address` | The SAC token address |
| `stream_counter()` | `u64` | Number of streams created |
| `get_total_supply()` | `i128` | Tokens currently in escrow across all active streams |
| `get_supply_cap()` | `i128` | Current global supply cap |
| `get_stream(id)` | `Stream` | Raw stream struct |
| `get_summary(id)` | `StreamSummary` | Bundled snapshot (total, vested, withdrawn, withdrawable, progress, status) |
| `get_status(id)` | `Status` | Lifecycle status |
| `is_active(id)` | `bool` | True if status is Active |
| `streamed_amount(id)` | `i128` | Vested amount at current timestamp |
| `withdrawable_amount(id)` | `i128` | Vested minus withdrawn |
| `remaining_amount(id)` | `i128` | Unvested amount |
| `duration(id)` | `u64` | Window length in seconds |
| `elapsed(id)` | `u64` | Seconds elapsed, clamped to window |
| `progress_bps(id)` | `u32` | Time progress in basis points (0–10 000) |
| `percent_withdrawn(id)` | `u32` | Withdrawal fraction in basis points |
See the README and the sources under src/ for the authoritative implementation.

## Batch creation

`create_stream_batch(sender, requests)` creates multiple streams from a single
sender. `requests` is a Soroban `Vec<StreamRequest>`; each request contains a
recipient, amount, start time, and end time. The operation requires the
sender's authorization once, validates the complete batch before escrow, and
returns the consecutive stream IDs. An empty or invalid batch fails atomically.
