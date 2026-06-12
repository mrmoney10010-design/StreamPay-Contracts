#![cfg(test)]
//! Unit tests for the StreamPay contract.

extern crate std;

use crate::error::Error;
use crate::types::Status;
use crate::{StreamPayContract, StreamPayContractClient};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{Address, Env};

/// Test fixture bundling the environment, contract client, token, and actors.
#[allow(dead_code)]
struct Setup<'a> {
    env: Env,
    contract: StreamPayContractClient<'a>,
    token: TokenClient<'a>,
    token_admin: StellarAssetClient<'a>,
    admin: Address,
    sender: Address,
    recipient: Address,
}

/// Builds a fully initialized contract with a funded sender.
fn setup<'a>() -> Setup<'a> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Deploy a Stellar Asset Contract to act as the streamed token.
    let issuer = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(issuer);
    let token = TokenClient::new(&env, &sac.address());
    let token_admin = StellarAssetClient::new(&env, &sac.address());

    let contract_id = env.register(StreamPayContract, ());
    let contract = StreamPayContractClient::new(&env, &contract_id);
    contract.initialize(&admin, &sac.address());

    // Fund the sender so it can escrow streams.
    token_admin.mint(&sender, &1_000_000);

    Setup {
        env,
        contract,
        token,
        token_admin,
        admin,
        sender,
        recipient,
    }
}

#[test]
fn test_initialize_sets_admin_and_token() {
    let s = setup();
    assert_eq!(s.contract.get_admin(), s.admin);
    assert_eq!(s.contract.get_token(), s.token.address);
    assert_eq!(s.contract.stream_counter(), 0);
}

#[test]
fn test_initialize_twice_fails() {
    let s = setup();
    let res = s.contract.try_initialize(&s.admin, &s.token.address);
    assert_eq!(res, Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn test_create_stream_escrows_and_returns_id() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);
    assert_eq!(id, 0);
    assert_eq!(s.contract.stream_counter(), 1);

    // The escrowed funds left the sender and now sit in the contract.
    assert_eq!(s.token.balance(&s.sender), 1_000_000 - 1_000);
    assert_eq!(s.token.balance(&s.contract.address), 1_000);

    let stream = s.contract.get_stream(&id);
    assert_eq!(stream.sender, s.sender);
    assert_eq!(stream.recipient, s.recipient);
    assert_eq!(stream.total, 1_000);
    assert_eq!(stream.withdrawn, 0);
    assert_eq!(stream.start, 100);
    assert_eq!(stream.end, 200);
    assert_eq!(stream.status, Status::Active);
}

#[test]
fn test_create_stream_rejects_zero_amount() {
    let s = setup();
    let res = s
        .contract
        .try_create_stream(&s.sender, &s.recipient, &0, &100, &200);
    assert_eq!(res, Err(Ok(Error::InvalidAmount)));
}

#[test]
fn test_create_stream_rejects_bad_time_range() {
    let s = setup();
    let res = s
        .contract
        .try_create_stream(&s.sender, &s.recipient, &1_000, &200, &100);
    assert_eq!(res, Err(Ok(Error::InvalidTimeRange)));
}

/// Sets the ledger timestamp used by time-based view functions.
fn set_time(env: &Env, ts: u64) {
    env.ledger().with_mut(|l| l.timestamp = ts);
}

#[test]
fn test_streamed_amount_zero_before_start() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);
    set_time(&s.env, 50);
    assert_eq!(s.contract.streamed_amount(&id), 0);
}

#[test]
fn test_streamed_amount_half_at_midpoint() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);
    // Halfway through the window should vest half of the total.
    set_time(&s.env, 150);
    assert_eq!(s.contract.streamed_amount(&id), 500);
}

#[test]
fn test_streamed_amount_full_after_end() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);
    set_time(&s.env, 250);
    assert_eq!(s.contract.streamed_amount(&id), 1_000);
}

#[test]
fn test_streamed_amount_quarter() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);
    // One quarter through the window vests a quarter of the total.
    set_time(&s.env, 125);
    assert_eq!(s.contract.streamed_amount(&id), 250);
}

#[test]
fn test_withdraw_transfers_vested_portion() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    set_time(&s.env, 150);
    let paid = s.contract.withdraw(&id, &s.recipient);
    assert_eq!(paid, 500);
    assert_eq!(s.token.balance(&s.recipient), 500);
    assert_eq!(s.token.balance(&s.contract.address), 500);

    let stream = s.contract.get_stream(&id);
    assert_eq!(stream.withdrawn, 500);
    assert_eq!(stream.status, Status::Active);
}

#[test]
fn test_double_withdraw_only_pays_new_vesting() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    set_time(&s.env, 150);
    assert_eq!(s.contract.withdraw(&id, &s.recipient), 500);

    // A second withdraw at the same time has nothing new to pay.
    let res = s.contract.try_withdraw(&id, &s.recipient);
    assert_eq!(res, Err(Ok(Error::NothingToWithdraw)));

    // Advancing time lets the recipient pull only the newly vested amount.
    set_time(&s.env, 175);
    assert_eq!(s.contract.withdraw(&id, &s.recipient), 250);
    assert_eq!(s.token.balance(&s.recipient), 750);
}

#[test]
fn test_withdraw_full_after_end_completes_stream() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    set_time(&s.env, 300);
    assert_eq!(s.contract.withdraw(&id, &s.recipient), 1_000);
    assert_eq!(s.token.balance(&s.recipient), 1_000);

    let stream = s.contract.get_stream(&id);
    assert_eq!(stream.status, Status::Completed);
}

#[test]
fn test_withdraw_by_non_recipient_fails() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    set_time(&s.env, 150);
    let stranger = Address::generate(&s.env);
    let res = s.contract.try_withdraw(&id, &stranger);
    assert_eq!(res, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_cancel_splits_funds_between_parties() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    // Cancel at the midpoint: recipient gets 500 vested, sender refunds 500.
    set_time(&s.env, 150);
    s.contract.cancel(&id, &s.sender);

    assert_eq!(s.token.balance(&s.recipient), 500);
    assert_eq!(s.token.balance(&s.sender), 1_000_000 - 500);
    assert_eq!(s.token.balance(&s.contract.address), 0);

    let stream = s.contract.get_stream(&id);
    assert_eq!(stream.status, Status::Cancelled);
}
