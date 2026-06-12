#![no_std]
//! StreamPay: a real-time payment-streaming smart contract for Stellar.
//!
//! StreamPay lets a sender escrow a fixed amount of a token and stream it
//! linearly to a recipient over a time window. The recipient can withdraw the
//! vested portion at any time, and either party can cancel an active stream.

pub mod error;
mod events;
mod storage;
pub mod types;
mod vesting;

#[cfg(test)]
mod test;

use crate::error::Error;
use crate::types::{Status, Stream};
use soroban_sdk::{contract, contractimpl, contractmeta, token, Address, Env};

// Embed human-readable metadata into the compiled contract.
contractmeta!(key = "name", val = "StreamPay");
contractmeta!(
    key = "desc",
    val = "Real-time linear payment-streaming contract for Stellar."
);
contractmeta!(key = "version", val = "0.1.0");

/// The smallest `total_amount` accepted by [`StreamPayContract::create_stream`].
///
/// Requiring a minimum avoids dust streams whose per-second vesting truncates
/// to zero and that only bloat persistent storage.
pub const MIN_STREAM_AMOUNT: i128 = 1;

/// The StreamPay contract type.
#[contract]
pub struct StreamPayContract;

#[contractimpl]
impl StreamPayContract {
    /// Initializes the contract with an `admin` and the streamed `token` (SAC).
    ///
    /// Can only be called once; subsequent calls return
    /// [`Error::AlreadyInitialized`].
    pub fn initialize(env: Env, admin: Address, token: Address) -> Result<(), Error> {
        if storage::has_admin(&env) {
            return Err(Error::AlreadyInitialized);
        }
        storage::write_admin(&env, &admin);
        storage::write_token(&env, &token);
        storage::write_counter(&env, 0);
        storage::extend_instance(&env);
        Ok(())
    }

    /// Returns the admin address, or [`Error::NotInitialized`] if unset.
    pub fn get_admin(env: Env) -> Result<Address, Error> {
        if !storage::has_admin(&env) {
            return Err(Error::NotInitialized);
        }
        Ok(storage::read_admin(&env))
    }

    /// Returns the streamed token address, or [`Error::NotInitialized`].
    pub fn get_token(env: Env) -> Result<Address, Error> {
        if !storage::has_admin(&env) {
            return Err(Error::NotInitialized);
        }
        Ok(storage::read_token(&env))
    }

    /// Returns the current stream counter (number of streams created).
    pub fn stream_counter(env: Env) -> u64 {
        storage::read_counter(&env)
    }

    /// Returns the stream with the given `id`, or [`Error::StreamNotFound`].
    pub fn get_stream(env: Env, id: u64) -> Result<Stream, Error> {
        storage::read_stream(&env, id).ok_or(Error::StreamNotFound)
    }

    /// Creates a new linear payment stream and escrows `total_amount`.
    ///
    /// The `sender` must authorize the call. `total_amount` is transferred from
    /// the sender into the contract immediately. Vesting runs linearly from
    /// `start_time` to `end_time`. Returns the new stream's id.
    pub fn create_stream(
        env: Env,
        sender: Address,
        recipient: Address,
        total_amount: i128,
        start_time: u64,
        end_time: u64,
    ) -> Result<u64, Error> {
        if !storage::has_admin(&env) {
            return Err(Error::NotInitialized);
        }
        sender.require_auth();

        if total_amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if total_amount < MIN_STREAM_AMOUNT {
            return Err(Error::AmountBelowMinimum);
        }
        if end_time <= start_time {
            return Err(Error::InvalidTimeRange);
        }
        // Reject streams that would already be fully vested on creation.
        if end_time <= env.ledger().timestamp() {
            return Err(Error::EndTimeInPast);
        }

        // Pull the escrowed funds from the sender into the contract.
        let token = storage::read_token(&env);
        let client = token::Client::new(&env, &token);
        client.transfer(&sender, &env.current_contract_address(), &total_amount);

        let id = storage::read_counter(&env);
        let next = id.checked_add(1).ok_or(Error::Overflow)?;

        let stream = Stream {
            sender: sender.clone(),
            recipient: recipient.clone(),
            total: total_amount,
            withdrawn: 0,
            start: start_time,
            end: end_time,
            status: Status::Active,
        };

        storage::write_stream(&env, id, &stream);
        storage::write_counter(&env, next);
        storage::extend_instance(&env);

        events::stream_created(&env, id, &sender, &recipient, total_amount);
        Ok(id)
    }

    /// Returns the length of stream `id`'s vesting window in seconds.
    ///
    /// This is `end - start` and is always positive for a stored stream,
    /// because [`Self::create_stream`] rejects non-increasing time ranges.
    pub fn duration(env: Env, id: u64) -> Result<u64, Error> {
        let stream = storage::read_stream(&env, id).ok_or(Error::StreamNotFound)?;
        Ok(stream.end - stream.start)
    }

    /// Returns how many seconds of stream `id`'s window have elapsed at the
    /// current ledger timestamp.
    ///
    /// The value is clamped to `[0, duration(id)]`.
    pub fn elapsed(env: Env, id: u64) -> Result<u64, Error> {
        let stream = storage::read_stream(&env, id).ok_or(Error::StreamNotFound)?;
        let now = env.ledger().timestamp();
        Ok(vesting::elapsed(&stream, now))
    }

    /// Returns the amount vested so far for stream `id` based on the current
    /// ledger timestamp.
    ///
    /// The result is `0` before `start`, `total` at or after `end`, and a
    /// linear interpolation in between.
    pub fn streamed_amount(env: Env, id: u64) -> Result<i128, Error> {
        let stream = storage::read_stream(&env, id).ok_or(Error::StreamNotFound)?;
        let now = env.ledger().timestamp();
        vesting::vested(&stream, now)
    }

    /// Returns the stream's vesting progress in basis points (0..=10_000).
    ///
    /// `0` means vesting has not started, `10_000` means it is complete. This
    /// reflects elapsed time only and is independent of the streamed `total`.
    pub fn progress_bps(env: Env, id: u64) -> Result<u32, Error> {
        let stream = storage::read_stream(&env, id).ok_or(Error::StreamNotFound)?;
        let now = env.ledger().timestamp();
        Ok(vesting::progress_bps(&stream, now))
    }

    /// Returns the amount of stream `id` that has not yet vested.
    ///
    /// This is `total - streamed_amount(id)` at the current ledger timestamp:
    /// the portion still locked in the contract that would be refunded to the
    /// sender on cancellation.
    pub fn remaining_amount(env: Env, id: u64) -> Result<i128, Error> {
        let stream = storage::read_stream(&env, id).ok_or(Error::StreamNotFound)?;
        let now = env.ledger().timestamp();
        vesting::unvested(&stream, now)
    }

    /// Returns the amount currently available to withdraw for stream `id`.
    ///
    /// This is the vested amount minus what the recipient has already
    /// withdrawn, i.e. the value that a [`Self::withdraw`] call would transfer
    /// at the current ledger timestamp.
    pub fn withdrawable_amount(env: Env, id: u64) -> Result<i128, Error> {
        let stream = storage::read_stream(&env, id).ok_or(Error::StreamNotFound)?;
        let now = env.ledger().timestamp();
        let vested = vesting::vested(&stream, now)?;
        let available = vested.checked_sub(stream.withdrawn).ok_or(Error::Overflow)?;
        Ok(if available > 0 { available } else { 0 })
    }

    /// Withdraws the vested-but-unwithdrawn balance of stream `id` to its
    /// `recipient`.
    ///
    /// The `recipient` must authorize the call. Returns the amount transferred.
    /// Errors with [`Error::NothingToWithdraw`] when no funds are available.
    pub fn withdraw(env: Env, id: u64, recipient: Address) -> Result<i128, Error> {
        recipient.require_auth();

        let mut stream = storage::read_stream(&env, id).ok_or(Error::StreamNotFound)?;
        if stream.recipient != recipient {
            return Err(Error::Unauthorized);
        }
        if stream.status == Status::Cancelled {
            return Err(Error::AlreadyCancelled);
        }
        if stream.status == Status::Completed {
            return Err(Error::AlreadyCompleted);
        }

        let now = env.ledger().timestamp();
        let vested = vesting::vested(&stream, now)?;
        let available = vested.checked_sub(stream.withdrawn).ok_or(Error::Overflow)?;
        if available <= 0 {
            return Err(Error::NothingToWithdraw);
        }

        stream.withdrawn = stream
            .withdrawn
            .checked_add(available)
            .ok_or(Error::Overflow)?;
        if stream.withdrawn >= stream.total {
            stream.status = Status::Completed;
        }
        storage::write_stream(&env, id, &stream);

        let token = storage::read_token(&env);
        let client = token::Client::new(&env, &token);
        client.transfer(&env.current_contract_address(), &recipient, &available);

        events::stream_withdrawn(&env, id, &recipient, available);
        Ok(available)
    }

    /// Cancels an active stream.
    ///
    /// The `caller` must be the stream's sender or recipient and must authorize
    /// the call. At cancellation the recipient is paid the streamed-but-
    /// unwithdrawn portion and the sender is refunded the unstreamed remainder.
    pub fn cancel(env: Env, id: u64, caller: Address) -> Result<(), Error> {
        caller.require_auth();

        let mut stream = storage::read_stream(&env, id).ok_or(Error::StreamNotFound)?;
        if caller != stream.sender && caller != stream.recipient {
            return Err(Error::Unauthorized);
        }
        if stream.status == Status::Cancelled {
            return Err(Error::AlreadyCancelled);
        }

        let now = env.ledger().timestamp();
        let vested = vesting::vested(&stream, now)?;

        // Recipient is owed the vested portion they have not yet withdrawn.
        let recipient_paid = vested.checked_sub(stream.withdrawn).ok_or(Error::Overflow)?;
        // Sender reclaims everything that has not vested.
        let sender_refund = vesting::unvested(&stream, now)?;

        stream.withdrawn = vested;
        stream.status = Status::Cancelled;
        storage::write_stream(&env, id, &stream);

        let token = storage::read_token(&env);
        let client = token::Client::new(&env, &token);
        let contract = env.current_contract_address();
        if recipient_paid > 0 {
            client.transfer(&contract, &stream.recipient, &recipient_paid);
        }
        if sender_refund > 0 {
            client.transfer(&contract, &stream.sender, &sender_refund);
        }

        events::stream_cancelled(&env, id, &caller, sender_refund, recipient_paid);
        Ok(())
    }
}
