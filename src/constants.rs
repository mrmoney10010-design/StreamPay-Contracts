//! Compile-time limits and validation boundaries for the StreamPay contract.
//!
//! Centralizing the contract's tunable bounds here keeps the magic numbers out
//! of the business logic in `lib.rs` and makes the contract's acceptance
//! criteria easy to find and audit in one place.

use soroban_sdk::contracterror;

/// The smallest `total_amount` accepted by
/// [`crate::StreamPayContract::create_stream`].
///
/// Requiring a minimum avoids dust streams whose per-second vesting truncates
/// to zero and that only bloat persistent storage.
pub const MIN_STREAM_AMOUNT: i128 = 1;

/// The smallest amount (streamed or topped-up) the contract accepts at all.
///
/// Any non-positive amount is rejected with [`Error::InvalidAmount`] before the
/// more specific [`MIN_STREAM_AMOUNT`] floor is consulted. Centralizing the
/// boundary here documents that `0` (and negative amounts) are never valid.
pub const MIN_VALID_AMOUNT: i128 = 1;

/// Error variants returned when a proposed amount violates the contract's
/// limits.
///
/// These mirror the amount-related variants in the contract's main [`crate::Error`]
/// enum so that callers and the contract surface a single, consistent set of
/// failure codes. They are kept in this module to colocate the *boundary* with
/// the *constants* that define it.
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum LimitError {
    /// The provided amount is zero or negative.
    InvalidAmount = 4,
    /// The requested amount is below [`MIN_STREAM_AMOUNT`].
    AmountBelowMinimum = 12,
}

/// Returns `true` when `amount` clears both the non-positive guard and the
/// [`MIN_STREAM_AMOUNT`] floor, i.e. it is a valid escrow amount.
///
/// This is the single source of truth used by `create_stream` and `top_up`
/// so the two entry points enforce identical bounds.
pub fn is_valid_amount(amount: i128) -> bool {
    amount >= MIN_VALID_AMOUNT && amount >= MIN_STREAM_AMOUNT
}
