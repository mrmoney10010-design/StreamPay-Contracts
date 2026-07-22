//! Event publishing helpers for the StreamPay contract.
//!
//! Events let off-chain indexers track stream lifecycle changes. Each event is
//! published with a descriptive topic tuple and a relevant data payload.

use soroban_sdk::{symbol_short, Address, Env, Symbol};

/// Publishes an `admin_scheduled` event when an admin transfer is queued.
pub fn admin_transfer_scheduled(
    env: &Env,
    current_admin: &Address,
    pending_admin: &Address,
    execute_after: u64,
) {
    env.events().publish(
        (Symbol::new(env, "admin_scheduled"),),
        (current_admin.clone(), pending_admin.clone(), execute_after),
    );
}

/// Publishes an `admin_transfer` event when a timelocked transfer executes.
pub fn admin_transfer_executed(env: &Env, previous_admin: &Address, new_admin: &Address) {
    env.events().publish(
        (Symbol::new(env, "admin_transfer"),),
        (previous_admin.clone(), new_admin.clone()),
    );
}

/// Publishes an `admin_cancelled` event when a scheduled transfer is cancelled.
pub fn admin_transfer_cancelled(env: &Env, admin: &Address) {
    env.events()
        .publish((Symbol::new(env, "admin_cancelled"),), admin.clone());
}

/// Publishes a `created` event when a new stream is opened.
pub fn stream_created(env: &Env, id: u64, sender: &Address, recipient: &Address, total: i128) {
    let topics = (symbol_short!("created"), id);
    env.events()
        .publish(topics, (sender.clone(), recipient.clone(), total));
}

/// Publishes a `withdrawn` event when a recipient pulls vested funds.
pub fn stream_withdrawn(env: &Env, id: u64, recipient: &Address, amount: i128) {
    let topics = (symbol_short!("withdrawn"), id);
    env.events().publish(topics, (recipient.clone(), amount));
}

/// Publishes a `toppedup` event when a sender adds funds to a stream.
///
/// `amount` is the value escrowed by this top-up and `new_total` is the
/// stream's total after the increase.
pub fn stream_topped_up(env: &Env, id: u64, sender: &Address, amount: i128, new_total: i128) {
    let topics = (symbol_short!("toppedup"), id);
    env.events()
        .publish(topics, (sender.clone(), amount, new_total));
}

/// Publishes an `extended` event when a stream's end time is pushed back.
///
/// `old_end` and `new_end` are the previous and updated end timestamps.
pub fn stream_extended(env: &Env, id: u64, sender: &Address, old_end: u64, new_end: u64) {
    let topics = (symbol_short!("extended"), id);
    env.events()
        .publish(topics, (sender.clone(), old_end, new_end));
}

/// Publishes a `cancelled` event when a stream is cancelled.
///
/// `sender_refund` is the unstreamed remainder returned to the sender and
/// `recipient_paid` is the streamed-but-unwithdrawn amount paid to the
/// recipient at cancellation time.
pub fn stream_cancelled(
    env: &Env,
    id: u64,
    caller: &Address,
    sender_refund: i128,
    recipient_paid: i128,
) {
    let topics = (symbol_short!("cancelled"), id);
    env.events()
        .publish(topics, (caller.clone(), sender_refund, recipient_paid));
}

/// Publishes a `capadmin` event when the admin changes the supply cap.
///
/// `old_cap` and `new_cap` bracket the change so indexers can detect the
/// transition without re-reading state.
pub fn supply_cap_updated(env: &Env, admin: &Address, old_cap: i128, new_cap: i128) {
    let topics = (symbol_short!("capadmin"),);
    env.events()
        .publish(topics, (admin.clone(), old_cap, new_cap));
}
/// Publishes an `admin_set` event when the admin role is transferred.
pub fn admin_changed(env: &Env, old_admin: &Address, new_admin: &Address) {
    let topics = (symbol_short!("admin_set"),);
    env.events()
        .publish(topics, (old_admin.clone(), new_admin.clone()));
}

/// Publishes an `upgraded` event when the contract's Wasm code is upgraded.
pub fn contract_upgraded(env: &Env, new_wasm_hash: &soroban_sdk::BytesN<32>) {
    let topics = (symbol_short!("upgraded"),);
    env.events().publish(topics, new_wasm_hash.clone());
}
