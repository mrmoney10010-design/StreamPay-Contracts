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
use soroban_sdk::{contract, contractimpl, Address, Env};

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
}
