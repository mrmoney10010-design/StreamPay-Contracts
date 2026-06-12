//! Storage layout and access helpers for the StreamPay contract.
//!
//! Instance storage holds singleton configuration (admin, token, counter).
//! Persistent storage holds individual streams keyed by their id.

use soroban_sdk::{contracttype, Address, Env};

/// Number of ledgers (~6 days) used as the persistent storage bump threshold.
pub const BUMP_THRESHOLD: u32 = 100_000;
/// Number of ledgers (~30 days) used as the persistent storage extend amount.
pub const BUMP_EXTEND: u32 = 518_400;

/// Keys used to address values in contract storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// The admin address (instance).
    Admin,
    /// The streamed token SAC address (instance).
    Token,
    /// The monotonically increasing stream counter (instance).
    Counter,
    /// A stream stored by its id (persistent).
    Stream(u64),
}

/// Returns `true` if the contract has been initialized (admin is set).
pub fn has_admin(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Admin)
}

/// Reads the admin address from instance storage.
///
/// Panics if the admin has not been set; callers must guard with
/// [`has_admin`] or the `NotInitialized` error first.
pub fn read_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

/// Writes the admin address into instance storage.
pub fn write_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

/// Reads the streamed token address from instance storage.
pub fn read_token(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Token).unwrap()
}

/// Writes the streamed token address into instance storage.
pub fn write_token(env: &Env, token: &Address) {
    env.storage().instance().set(&DataKey::Token, token);
}
