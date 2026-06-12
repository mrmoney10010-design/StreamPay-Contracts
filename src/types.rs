//! Core data types for the StreamPay contract.

use soroban_sdk::{contracttype, Address};

/// Lifecycle status of a payment stream.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    /// The stream is active and vesting over time.
    Active = 0,
    /// The stream was cancelled before its end time.
    Cancelled = 1,
    /// The stream has been fully withdrawn.
    Completed = 2,
}

/// A computed, point-in-time snapshot of a stream's vesting figures.
///
/// Returned by view calls so off-chain clients can fetch the headline numbers
/// in a single round trip instead of combining several getters.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamSummary {
    /// The total amount escrowed for the stream.
    pub total: i128,
    /// The amount vested so far at the queried timestamp.
    pub vested: i128,
    /// The amount already withdrawn by the recipient.
    pub withdrawn: i128,
    /// The vested-but-unwithdrawn amount available to the recipient now.
    pub withdrawable: i128,
    /// Vesting progress in basis points (0..=10_000) by elapsed time.
    pub progress_bps: u32,
    /// The current lifecycle status of the stream.
    pub status: Status,
}

/// A linear payment stream.
///
/// Tokens vest linearly from `start` to `end`. The escrowed `total` is held by
/// the contract; `withdrawn` tracks how much the recipient has already pulled.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stream {
    /// The account funding the stream.
    pub sender: Address,
    /// The account receiving the streamed tokens.
    pub recipient: Address,
    /// The total amount escrowed for the stream.
    pub total: i128,
    /// The amount already withdrawn by the recipient.
    pub withdrawn: i128,
    /// The ledger timestamp at which vesting begins.
    pub start: u64,
    /// The ledger timestamp at which vesting completes.
    pub end: u64,
    /// The current lifecycle status of the stream.
    pub status: Status,
}
