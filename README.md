# StreamPay

StreamPay is a real-time payment-streaming smart contract for the
[Stellar](https://stellar.org) network, written in Rust with the
[Soroban SDK](https://developers.stellar.org/docs/build/smart-contracts).

A sender escrows a fixed amount of a token and the contract releases it to a
recipient **linearly over a time window**. The recipient can withdraw the
vested portion at any time, and either party can cancel an active stream to
split the funds fairly between what has and has not yet vested.

## Features

- Linear, time-based vesting of an escrowed token amount.
- On-demand withdrawals of the vested-but-unwithdrawn balance.
- Cancellation that refunds the unstreamed remainder to the sender and pays the
  streamed remainder to the recipient.
- Authorization enforced with `require_auth` on every state-changing call.
- Checked arithmetic throughout to avoid silent overflow.
- Events emitted for stream creation, withdrawal, and cancellation.

## Contract API

| Function | Description |
| --- | --- |
| `initialize(admin, token)` | One-time setup: records the admin and the streamed token (SAC). |
| `create_stream(sender, recipient, total_amount, start_time, end_time) -> u64` | Escrows `total_amount` from `sender` and opens a stream; returns its id. |
| `streamed_amount(id) -> i128` | View: amount vested so far based on the ledger timestamp. |
| `withdrawable_amount(id) -> i128` | View: vested-but-unwithdrawn balance available to the recipient right now. |
| `remaining_amount(id) -> i128` | View: amount not yet vested (the sender's potential refund). |
| `progress_bps(id) -> u32` | View: vesting progress in basis points (0..=10_000) by elapsed time. |
| `withdraw(id, recipient) -> i128` | Recipient pulls the vested-but-unwithdrawn balance; returns the amount paid. |
| `cancel(id, caller)` | Sender or recipient cancels; splits funds by vested/unvested. |
| `get_stream(id) -> Stream` | View: the full stream record. |
| `get_admin() -> Address` | View: the configured admin. |
| `get_token() -> Address` | View: the streamed token address. |
| `stream_counter() -> u64` | View: number of streams created so far. |

### Vesting

The vested amount at timestamp `t` is:

```
vested(t) = 0                                              if t <= start
vested(t) = total                                          if t >= end
vested(t) = total * (t - start) / (end - start)            otherwise
```

Integer division truncates, so dust may accrue at the end of the window; it is
always fully released once `t >= end`.

## Events

The contract publishes an event for every lifecycle change so off-chain
indexers can follow streams without polling. Each event's topics carry the
event name and the stream id.

| Topic | Data | Emitted by |
| --- | --- | --- |
| `("created", id)` | `(sender, recipient, total)` | `create_stream` |
| `("withdrawn", id)` | `(recipient, amount)` | `withdraw` |
| `("cancelled", id)` | `(caller, sender_refund, recipient_paid)` | `cancel` |

## Build

Install the Rust `wasm32-unknown-unknown` target and the
[Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli),
then:

```bash
make build      # compile the optimized release wasm
make test       # run the unit test suite
make fmt        # format the source tree
make clippy     # lint with warnings denied
```

The release artifact is written to:

```
target/wasm32-unknown-unknown/release/streampay_contract.wasm
```

## Deploy

```bash
# Optimize the wasm (optional but recommended).
make optimize

# Deploy to a network. Override SOURCE and NETWORK as needed.
make deploy NETWORK=testnet SOURCE=alice
```

After deploying, initialize the contract once with an admin and the address of
the Stellar Asset Contract (SAC) to stream:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source alice \
  --network testnet \
  -- initialize \
  --admin <ADMIN_ADDRESS> \
  --token <TOKEN_SAC_ADDRESS>
```

## License

Licensed under the [MIT License](LICENSE).
