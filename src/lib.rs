#![no_std]
//! StreamPay: a real-time payment-streaming smart contract for Stellar.
//!
//! StreamPay lets a sender escrow a fixed amount of a token and stream it
//! linearly to a recipient over a time window. The recipient can withdraw the
//! vested portion at any time, and either party can cancel an active stream.

mod error;
mod events;
mod storage;
mod types;
mod vesting;

use crate::error::Error;
use crate::types::{Status, Stream};
use soroban_sdk::{contract, contractimpl, token, Address, Env};

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
        if end_time <= start_time {
            return Err(Error::InvalidTimeRange);
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
}
