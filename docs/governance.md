# Governance

This note documents the **governance** of the streampay-contract contract.

streampay-contract is a Soroban smart contract on the Stellar network. This page is part of the
project's reference documentation and describes the governance in detail, covering the relevant
entrypoints, storage layout, and invariants where applicable.

See the README and the sources under src/ for the authoritative implementation.

## Timelocked administration

Administrative control changes use a one-day timelock. The current administrator
must authorize `schedule_admin_transfer(admin, new_admin)`. It creates (or
replaces) a pending transfer that becomes executable after 86,400 seconds of
ledger time. `execute_admin_transfer()` is permissionless after that time, and
`cancel_admin_transfer(admin)` lets the current administrator revoke the pending
transfer before it is executed. The contract exposes the pending address and
execution timestamp through read-only getters so off-chain monitoring can alert
users before an administrative change takes effect.
