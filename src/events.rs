//! Event publishing helpers for the StreamPay contract.
//!
//! Events let off-chain indexers track stream lifecycle changes. Each event is
//! published with a descriptive topic tuple and a relevant data payload.

use soroban_sdk::{symbol_short, Address, Env};

/// Publishes a `created` event when a new stream is opened.
pub fn stream_created(
    env: &Env,
    id: u64,
    sender: &Address,
    recipient: &Address,
    total: i128,
) {
    let topics = (symbol_short!("created"), id);
    env.events()
        .publish(topics, (sender.clone(), recipient.clone(), total));
}

/// Publishes a `withdrawn` event when a recipient pulls vested funds.
pub fn stream_withdrawn(env: &Env, id: u64, recipient: &Address, amount: i128) {
    let topics = (symbol_short!("withdrawn"), id);
    env.events().publish(topics, (recipient.clone(), amount));
}
