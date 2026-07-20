#![no_std]
//! StreamPay: a real-time payment-streaming smart contract for Stellar.
//!
//! StreamPay lets a sender escrow a fixed amount of a token and stream it
//! linearly to a recipient over a time window. The recipient can withdraw the
//! vested portion at any time, and either party can cancel an active stream.

pub mod constants;
pub mod error;
mod events;
pub mod normalize;
mod storage;
pub mod types;
mod vesting;

#[cfg(test)]
mod test;

use crate::error::Error;
use crate::types::{Status, Stream, StreamRequest, StreamSummary};
use soroban_sdk::{contract, contractimpl, contractmeta, token, Address, Env, Vec};

// Embed human-readable metadata into the compiled contract.
contractmeta!(key = "name", val = "StreamPay");
contractmeta!(
    key = "desc",
    val = "Real-time linear payment-streaming contract for Stellar."
);
contractmeta!(key = "version", val = "0.2.0");

/// Re-export of the contract's compile-time limits and validation helpers.
///
/// Contract-level validation and governance limits are declared in
/// [`crate::constants`]. Re-exporting them preserves convenient downstream
/// access through the crate root.
pub use crate::constants::{ADMIN_TIMELOCK_DELAY, MIN_STREAM_AMOUNT};

use crate::constants::{is_valid_amount, DEFAULT_SUPPLY_CAP};

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
        storage::write_total_supply(&env, 0);
        storage::write_supply_cap(&env, DEFAULT_SUPPLY_CAP);
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

    /// Returns the address in the scheduled admin transfer, if any.
    pub fn get_pending_admin(env: Env) -> Result<Option<Address>, Error> {
        if !storage::has_admin(&env) {
            return Err(Error::NotInitialized);
        }
        Ok(storage::read_pending_admin(&env))
    }

    /// Returns when the scheduled admin transfer may execute, if any.
    pub fn get_admin_action_execute_after(env: Env) -> Result<Option<u64>, Error> {
        if !storage::has_admin(&env) {
            return Err(Error::NotInitialized);
        }
        Ok(storage::read_admin_action_execute_after(&env))
    }

    /// Schedules a transfer of the admin role after [`ADMIN_TIMELOCK_DELAY`].
    ///
    /// Only the current admin may schedule the transfer. Scheduling a new
    /// transfer replaces a previously scheduled one. Once the delay has
    /// elapsed, anyone may call [`Self::execute_admin_transfer`] to execute it.
    pub fn schedule_admin_transfer(
        env: Env,
        admin: Address,
        new_admin: Address,
    ) -> Result<u64, Error> {
        if !storage::has_admin(&env) {
            return Err(Error::NotInitialized);
        }
        admin.require_auth();
        if admin != storage::read_admin(&env) {
            return Err(Error::Unauthorized);
        }
        if new_admin == admin {
            return Err(Error::InvalidAdminAction);
        }

        let execute_after = env
            .ledger()
            .timestamp()
            .checked_add(ADMIN_TIMELOCK_DELAY)
            .ok_or(Error::Overflow)?;
        storage::write_pending_admin_action(&env, &new_admin, execute_after);
        storage::extend_instance(&env);
        events::admin_transfer_scheduled(&env, &admin, &new_admin, execute_after);
        Ok(execute_after)
    }

    /// Executes the scheduled admin transfer after its timelock has elapsed.
    ///
    /// Execution is permissionless so an approved governance change cannot be
    /// blocked if the current admin becomes unavailable.
    pub fn execute_admin_transfer(env: Env) -> Result<(), Error> {
        if !storage::has_admin(&env) {
            return Err(Error::NotInitialized);
        }
        let pending_admin = storage::read_pending_admin(&env).ok_or(Error::NoPendingAdminAction)?;
        let execute_after = storage::read_admin_action_execute_after(&env)
            .ok_or(Error::NoPendingAdminAction)?;
        if env.ledger().timestamp() < execute_after {
            return Err(Error::TimelockNotExpired);
        }

        let previous_admin = storage::read_admin(&env);
        storage::write_admin(&env, &pending_admin);
        storage::clear_pending_admin_action(&env);
        storage::extend_instance(&env);
        events::admin_transfer_executed(&env, &previous_admin, &pending_admin);
        Ok(())
    }

    /// Cancels the currently scheduled admin transfer.
    ///
    /// Only the current admin may cancel a pending transfer.
    pub fn cancel_admin_transfer(env: Env, admin: Address) -> Result<(), Error> {
        if !storage::has_admin(&env) {
            return Err(Error::NotInitialized);
        }
        admin.require_auth();
        if admin != storage::read_admin(&env) {
            return Err(Error::Unauthorized);
        }
        if storage::read_pending_admin(&env).is_none() {
            return Err(Error::NoPendingAdminAction);
        }

        storage::clear_pending_admin_action(&env);
        storage::extend_instance(&env);
        events::admin_transfer_cancelled(&env, &admin);
        Ok(())
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

    /// Returns the total amount currently held in escrow across all active
    /// streams.
    ///
    /// This value is incremented when `create_stream` or `top_up` transfers
    /// tokens into the contract, and decremented when tokens leave via
    /// `withdraw` (completion) or `cancel` (refund + vested payout).
    pub fn get_total_supply(env: Env) -> i128 {
        storage::read_total_supply(&env)
    }

    /// Returns the current supply cap.
    ///
    /// `create_stream` and `top_up` both fail with
    /// [`Error::SupplyCapExceeded`] if the resulting total supply would exceed
    /// this value.
    pub fn get_supply_cap(env: Env) -> i128 {
        storage::read_supply_cap(&env)
    }

    /// Updates the global supply cap to `new_cap`.
    ///
    /// Only the admin may call this. `new_cap` must be positive (> 0); setting
    /// it below the current `total_supply` does not affect existing streams but
    /// will prevent new ones from being created until supply drops below the
    /// new cap. Emits a `capadmin` event on success.
    pub fn set_supply_cap(env: Env, new_cap: i128) -> Result<(), Error> {
        if !storage::has_admin(&env) {
            return Err(Error::NotInitialized);
        }
        let admin = storage::read_admin(&env);
        admin.require_auth();

        if new_cap <= 0 {
            return Err(Error::InvalidAmount);
        }

        let old_cap = storage::read_supply_cap(&env);
        storage::write_supply_cap(&env, new_cap);
        storage::extend_instance(&env);

        events::supply_cap_updated(&env, &admin, old_cap, new_cap);
        Ok(())
    }

    /// Returns the stream with the given `id`, or [`Error::StreamNotFound`].
    pub fn get_stream(env: Env, id: u64) -> Result<Stream, Error> {
        storage::read_stream(&env, id).ok_or(Error::StreamNotFound)
    }

    /// Creates a new linear payment stream and escrows `total_amount`.
    ///
    /// The `sender` must authorize the call. `total_amount` is transferred from
    /// the sender into the contract immediately. Vesting runs linearly from
    /// `start_time` to `end_time`. A `start_time` of `0` is shorthand for the
    /// current ledger timestamp (see [`crate::normalize::normalize_start_time`]),
    /// so the stream starts vesting immediately. Returns the new stream's id.
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

        // Canonicalize raw inputs before validating them.
        let start_time = normalize::normalize_start_time(env.ledger().timestamp(), start_time);

        if !is_valid_amount(total_amount) {
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

        // Check the global supply cap before pulling any tokens.
        let current_supply = storage::read_total_supply(&env);
        let new_supply = current_supply
            .checked_add(total_amount)
            .ok_or(Error::Overflow)?;
        let cap = storage::read_supply_cap(&env);
        if new_supply > cap {
            return Err(Error::SupplyCapExceeded);
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
        storage::write_total_supply(&env, new_supply);
        storage::extend_instance(&env);

        events::stream_created(&env, id, &sender, &recipient, total_amount);
        Ok(id)
    }

    /// Creates multiple streams funded by one `sender` in a single atomic
    /// invocation and returns their consecutive ids.
    ///
    /// The sender authorizes once. Every request is validated before the
    /// aggregate escrow transfer occurs, so an invalid request leaves no
    /// partial streams or token transfers behind. Each created stream emits
    /// the normal `created` event.
    pub fn create_stream_batch(
        env: Env,
        sender: Address,
        requests: Vec<StreamRequest>,
    ) -> Result<Vec<u64>, Error> {
        if !storage::has_admin(&env) {
            return Err(Error::NotInitialized);
        }
        sender.require_auth();
        if requests.is_empty() {
            return Err(Error::EmptyBatch);
        }

        let now = env.ledger().timestamp();
        let mut aggregate_amount = 0_i128;
        let mut index = 0_u32;
        while index < requests.len() {
            let request = requests.get(index).unwrap();
            if !is_valid_amount(request.total_amount) {
                return Err(Error::InvalidAmount);
            }
            if request.total_amount < MIN_STREAM_AMOUNT {
                return Err(Error::AmountBelowMinimum);
            }
            if request.end_time <= request.start_time {
                return Err(Error::InvalidTimeRange);
            }
            if request.end_time <= now {
                return Err(Error::EndTimeInPast);
            }
            aggregate_amount = aggregate_amount
                .checked_add(request.total_amount)
                .ok_or(Error::Overflow)?;
            index += 1;
        }

        let count = requests.len() as u64;
        let first_id = storage::read_counter(&env);
        let next_counter = first_id.checked_add(count).ok_or(Error::Overflow)?;

        let token = storage::read_token(&env);
        token::Client::new(&env, &token).transfer(
            &sender,
            &env.current_contract_address(),
            &aggregate_amount,
        );

        let mut ids = Vec::new(&env);
        index = 0;
        while index < requests.len() {
            let request = requests.get(index).unwrap();
            let id = first_id + index as u64;
            let stream = Stream {
                sender: sender.clone(),
                recipient: request.recipient.clone(),
                total: request.total_amount,
                withdrawn: 0,
                start: request.start_time,
                end: request.end_time,
                status: Status::Active,
            };
            storage::write_stream(&env, id, &stream);
            events::stream_created(&env, id, &sender, &request.recipient, request.total_amount);
            ids.push_back(id);
            index += 1;
        }
        storage::write_counter(&env, next_counter);
        storage::extend_instance(&env);
        Ok(ids)
    }

    /// Adds `amount` more tokens to an active stream `id` and escrows them.
    ///
    /// Only the stream's `sender` may top up, and they must authorize the call.
    /// The extra funds vest over the same window, increasing the per-second
    /// rate. Returns the stream's new total. Errors with
    /// [`Error::StreamNotActive`] if the stream is cancelled or completed.
    pub fn top_up(env: Env, id: u64, sender: Address, amount: i128) -> Result<i128, Error> {
        sender.require_auth();

        if !is_valid_amount(amount) {
            return Err(Error::InvalidAmount);
        }

        let mut stream = storage::read_stream(&env, id).ok_or(Error::StreamNotFound)?;
        if sender != stream.sender {
            return Err(Error::Unauthorized);
        }
        if stream.status != Status::Active {
            return Err(Error::StreamNotActive);
        }

        let new_total = stream.total.checked_add(amount).ok_or(Error::Overflow)?;

        // Check the global supply cap before pulling additional tokens.
        let current_supply = storage::read_total_supply(&env);
        let new_supply = current_supply
            .checked_add(amount)
            .ok_or(Error::Overflow)?;
        let cap = storage::read_supply_cap(&env);
        if new_supply > cap {
            return Err(Error::SupplyCapExceeded);
        }

        let token = storage::read_token(&env);
        let client = token::Client::new(&env, &token);
        client.transfer(&sender, &env.current_contract_address(), &amount);

        stream.total = new_total;
        storage::write_stream(&env, id, &stream);
        storage::write_total_supply(&env, new_supply);
        storage::extend_instance(&env);

        events::stream_topped_up(&env, id, &sender, amount, new_total);
        Ok(new_total)
    }

    /// Pushes back the `end` time of an active stream `id` to `new_end`.
    ///
    /// Only the stream's `sender` may extend, and they must authorize the call.
    /// Extending lowers the per-second vesting rate by spreading the same total
    /// over a longer window. `new_end` must be strictly later than the current
    /// end, otherwise [`Error::InvalidTimeRange`] is returned. Errors with
    /// [`Error::StreamNotActive`] if the stream is cancelled or completed.
    pub fn extend_stream(env: Env, id: u64, sender: Address, new_end: u64) -> Result<(), Error> {
        sender.require_auth();

        let mut stream = storage::read_stream(&env, id).ok_or(Error::StreamNotFound)?;
        if sender != stream.sender {
            return Err(Error::Unauthorized);
        }
        if stream.status != Status::Active {
            return Err(Error::StreamNotActive);
        }
        if new_end <= stream.end {
            return Err(Error::InvalidTimeRange);
        }

        let old_end = stream.end;
        stream.end = new_end;
        storage::write_stream(&env, id, &stream);
        storage::extend_instance(&env);

        events::stream_extended(&env, id, &sender, old_end, new_end);
        Ok(())
    }

    /// Returns the lifecycle [`Status`] of stream `id`.
    pub fn get_status(env: Env, id: u64) -> Result<Status, Error> {
        let stream = storage::read_stream(&env, id).ok_or(Error::StreamNotFound)?;
        Ok(stream.status)
    }

    /// Returns `true` if stream `id` is still active (not cancelled or
    /// completed).
    pub fn is_active(env: Env, id: u64) -> Result<bool, Error> {
        let stream = storage::read_stream(&env, id).ok_or(Error::StreamNotFound)?;
        Ok(stream.status == Status::Active)
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

    /// Returns the share of stream `id`'s total that has been withdrawn, in
    /// basis points (0..=10_000).
    ///
    /// `0` means nothing has been pulled yet and `10_000` means the recipient
    /// has withdrawn the entire escrowed total. Unlike [`Self::progress_bps`],
    /// this reflects withdrawals rather than elapsed time.
    pub fn percent_withdrawn(env: Env, id: u64) -> Result<u32, Error> {
        let stream = storage::read_stream(&env, id).ok_or(Error::StreamNotFound)?;
        if stream.total <= 0 {
            return Ok(0);
        }
        let bps = stream
            .withdrawn
            .checked_mul(10_000)
            .ok_or(Error::Overflow)?
            / stream.total;
        Ok(bps as u32)
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

    /// Returns a [`StreamSummary`] of stream `id` at the current ledger
    /// timestamp.
    ///
    /// This bundles the total, vested, withdrawn, withdrawable, progress, and
    /// status so a client can read them all in one call.
    pub fn get_summary(env: Env, id: u64) -> Result<StreamSummary, Error> {
        let stream = storage::read_stream(&env, id).ok_or(Error::StreamNotFound)?;
        let now = env.ledger().timestamp();
        Ok(StreamSummary {
            total: stream.total,
            vested: vesting::vested(&stream, now)?,
            withdrawn: stream.withdrawn,
            withdrawable: vesting::withdrawable(&stream, now)?,
            progress_bps: vesting::progress_bps(&stream, now),
            status: stream.status,
        })
    }

    /// Returns the amount currently available to withdraw for stream `id`.
    ///
    /// This is the vested amount minus what the recipient has already
    /// withdrawn, i.e. the value that a [`Self::withdraw`] call would transfer
    /// at the current ledger timestamp.
    pub fn withdrawable_amount(env: Env, id: u64) -> Result<i128, Error> {
        let stream = storage::read_stream(&env, id).ok_or(Error::StreamNotFound)?;
        let now = env.ledger().timestamp();
        vesting::withdrawable(&stream, now)
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
        let available = vesting::withdrawable(&stream, now)?;
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

        // Tokens have left the contract; reduce the tracked supply.
        let supply = storage::read_total_supply(&env);
        let new_supply = supply.checked_sub(available).unwrap_or(0);
        storage::write_total_supply(&env, new_supply);

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
        let recipient_paid = vested
            .checked_sub(stream.withdrawn)
            .ok_or(Error::Overflow)?;
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

        // Both payouts have left the contract; reduce the tracked supply by
        // the entire remaining escrow (unvested + vested-but-unwithdrawn).
        let released = recipient_paid
            .checked_add(sender_refund)
            .unwrap_or(sender_refund);
        let supply = storage::read_total_supply(&env);
        let new_supply = supply.checked_sub(released).unwrap_or(0);
        storage::write_total_supply(&env, new_supply);

        events::stream_cancelled(&env, id, &caller, sender_refund, recipient_paid);
        Ok(())
    }
}
