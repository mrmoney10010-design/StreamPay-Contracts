//! Core data types for the StreamPay contract.

use soroban_sdk::contracttype;

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
