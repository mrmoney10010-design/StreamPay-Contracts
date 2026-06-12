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
