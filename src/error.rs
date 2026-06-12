//! Error types returned by the StreamPay contract.

use soroban_sdk::contracterror;

/// Errors that the StreamPay contract can return to callers.
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// The contract has already been initialized.
    AlreadyInitialized = 1,
    /// The contract has not been initialized yet.
    NotInitialized = 2,
    /// No stream exists for the given id.
    StreamNotFound = 3,
    /// The provided amount is zero or negative.
    InvalidAmount = 4,
    /// The start/end time range is invalid.
    InvalidTimeRange = 5,
    /// The caller is not authorized for this action.
    Unauthorized = 6,
    /// An arithmetic operation overflowed.
    Overflow = 7,
    /// The stream has already been cancelled.
    AlreadyCancelled = 8,
    /// There is nothing available to withdraw.
    NothingToWithdraw = 9,
}
