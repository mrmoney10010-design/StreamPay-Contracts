#![cfg(test)]
//! Unit tests for the StreamPay contract.

extern crate std;

use crate::error::Error;
use crate::types::Status;
use crate::{StreamPayContract, StreamPayContractClient};
use soroban_sdk::testutils::{Address as _, AuthorizedFunction, Ledger};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{Address, Env, IntoVal, Symbol};

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
fn test_admin_transfer_requires_timelock_then_executes() {
    let s = setup();
    let new_admin = Address::generate(&s.env);
    set_time(&s.env, 1_000);

    let execute_after = s
        .contract
        .schedule_admin_transfer(&s.admin, &new_admin);
    assert_eq!(execute_after, 1_000 + crate::ADMIN_TIMELOCK_DELAY);
    assert_eq!(s.contract.get_pending_admin(), Some(new_admin.clone()));
    assert_eq!(s.contract.get_admin_action_execute_after(), Some(execute_after));
    assert_eq!(
        s.contract.try_execute_admin_transfer(),
        Err(Ok(Error::TimelockNotExpired))
    );

    set_time(&s.env, execute_after);
    // The executor is intentionally permissionless after the delay.
    s.contract.execute_admin_transfer();
    assert_eq!(s.contract.get_admin(), new_admin);
    assert_eq!(s.contract.get_pending_admin(), None);
    assert_eq!(s.contract.get_admin_action_execute_after(), None);
}

#[test]
fn test_admin_transfer_can_be_replaced_or_cancelled_by_current_admin() {
    let s = setup();
    let first = Address::generate(&s.env);
    let replacement = Address::generate(&s.env);
    s.contract.schedule_admin_transfer(&s.admin, &first);
    s.contract.schedule_admin_transfer(&s.admin, &replacement);
    assert_eq!(s.contract.get_pending_admin(), Some(replacement));

    s.contract.cancel_admin_transfer(&s.admin);
    assert_eq!(s.contract.get_pending_admin(), None);
    assert_eq!(
        s.contract.try_execute_admin_transfer(),
        Err(Ok(Error::NoPendingAdminAction))
    );
}

#[test]
fn test_admin_transfer_rejects_non_admin_and_noop_transfer() {
    let s = setup();
    let stranger = Address::generate(&s.env);
    let new_admin = Address::generate(&s.env);
    assert_eq!(
        s.contract.try_schedule_admin_transfer(&stranger, &new_admin),
        Err(Ok(Error::Unauthorized))
    );
    assert_eq!(
        s.contract.try_schedule_admin_transfer(&s.admin, &s.admin),
        Err(Ok(Error::InvalidAdminAction))
    );
    assert_eq!(
        s.contract.try_cancel_admin_transfer(&s.admin),
        Err(Ok(Error::NoPendingAdminAction))
    );
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
fn test_get_summary_bundles_figures() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    set_time(&s.env, 150);
    let summary = s.contract.get_summary(&id);
    assert_eq!(summary.total, 1_000);
    assert_eq!(summary.vested, 500);
    assert_eq!(summary.withdrawn, 0);
    assert_eq!(summary.withdrawable, 500);
    assert_eq!(summary.progress_bps, 5_000);
    assert_eq!(summary.status, Status::Active);
}

#[test]
fn test_create_stream_rejects_past_end_time() {
    let s = setup();
    // Advance the ledger past the proposed end time.
    set_time(&s.env, 500);
    let res = s
        .contract
        .try_create_stream(&s.sender, &s.recipient, &1_000, &100, &200);
    assert_eq!(res, Err(Ok(Error::EndTimeInPast)));
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
fn test_withdrawable_amount_tracks_withdrawals() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    // Before start nothing is withdrawable.
    set_time(&s.env, 50);
    assert_eq!(s.contract.withdrawable_amount(&id), 0);

    // At the midpoint the full vested half is withdrawable.
    set_time(&s.env, 150);
    assert_eq!(s.contract.withdrawable_amount(&id), 500);

    // After withdrawing, the withdrawable amount drops back to zero.
    s.contract.withdraw(&id, &s.recipient);
    assert_eq!(s.contract.withdrawable_amount(&id), 0);
}

#[test]
fn test_remaining_amount_complements_streamed() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    set_time(&s.env, 125);
    // remaining + streamed always equals the escrowed total.
    assert_eq!(s.contract.streamed_amount(&id), 250);
    assert_eq!(s.contract.remaining_amount(&id), 750);

    set_time(&s.env, 250);
    assert_eq!(s.contract.remaining_amount(&id), 0);
}

#[test]
fn test_duration_returns_window_length() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);
    assert_eq!(s.contract.duration(&id), 100);
}

#[test]
fn test_elapsed_clamps_to_window() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    // Before start no time has elapsed.
    set_time(&s.env, 50);
    assert_eq!(s.contract.elapsed(&id), 0);
    // Partway through reports the seconds since start.
    set_time(&s.env, 175);
    assert_eq!(s.contract.elapsed(&id), 75);
    // Past the end it saturates at the full window length.
    set_time(&s.env, 500);
    assert_eq!(s.contract.elapsed(&id), 100);
}

#[test]
fn test_progress_bps_reports_time_fraction() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    set_time(&s.env, 50);
    assert_eq!(s.contract.progress_bps(&id), 0);
    set_time(&s.env, 150);
    assert_eq!(s.contract.progress_bps(&id), 5_000);
    set_time(&s.env, 250);
    assert_eq!(s.contract.progress_bps(&id), 10_000);
}

#[test]
fn test_percent_withdrawn_tracks_total_pulled() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    // Nothing withdrawn yet.
    assert_eq!(s.contract.percent_withdrawn(&id), 0);

    // Withdraw half the total at the midpoint -> 5_000 bps.
    set_time(&s.env, 150);
    s.contract.withdraw(&id, &s.recipient);
    assert_eq!(s.contract.percent_withdrawn(&id), 5_000);

    // Withdraw the rest after the end -> 10_000 bps.
    set_time(&s.env, 250);
    s.contract.withdraw(&id, &s.recipient);
    assert_eq!(s.contract.percent_withdrawn(&id), 10_000);
}

#[test]
fn test_status_views_track_lifecycle() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    assert!(s.contract.is_active(&id));
    assert_eq!(s.contract.get_status(&id), Status::Active);

    set_time(&s.env, 150);
    s.contract.cancel(&id, &s.sender);

    assert!(!s.contract.is_active(&id));
    assert_eq!(s.contract.get_status(&id), Status::Cancelled);
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
fn test_withdraw_after_completion_fails() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    // Fully withdraw after the end, completing the stream.
    set_time(&s.env, 300);
    assert_eq!(s.contract.withdraw(&id, &s.recipient), 1_000);

    // A further withdraw is rejected as already completed.
    let res = s.contract.try_withdraw(&id, &s.recipient);
    assert_eq!(res, Err(Ok(Error::AlreadyCompleted)));
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
fn test_top_up_increases_total_and_escrow() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    let new_total = s.contract.top_up(&id, &s.sender, &500);
    assert_eq!(new_total, 1_500);
    assert_eq!(s.contract.get_stream(&id).total, 1_500);

    // The extra escrow moved from the sender into the contract.
    assert_eq!(s.token.balance(&s.contract.address), 1_500);
    assert_eq!(s.token.balance(&s.sender), 1_000_000 - 1_500);

    // The larger total vests over the same window, so the midpoint is 750.
    set_time(&s.env, 150);
    assert_eq!(s.contract.streamed_amount(&id), 750);
}

#[test]
fn test_top_up_rejects_non_sender_and_bad_amount() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    let stranger = Address::generate(&s.env);
    assert_eq!(
        s.contract.try_top_up(&id, &stranger, &500),
        Err(Ok(Error::Unauthorized))
    );
    assert_eq!(
        s.contract.try_top_up(&id, &s.sender, &0),
        Err(Ok(Error::InvalidAmount))
    );
}

#[test]
fn test_top_up_rejects_cancelled_stream() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    set_time(&s.env, 150);
    s.contract.cancel(&id, &s.sender);

    assert_eq!(
        s.contract.try_top_up(&id, &s.sender, &500),
        Err(Ok(Error::StreamNotActive))
    );
}

#[test]
fn test_extend_stream_slows_vesting() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    // Double the window: end moves from 200 to 300.
    s.contract.extend_stream(&id, &s.sender, &300);
    assert_eq!(s.contract.get_stream(&id).end, 300);
    assert_eq!(s.contract.duration(&id), 200);

    // At t=150 the original midpoint now vests only a quarter.
    set_time(&s.env, 150);
    assert_eq!(s.contract.streamed_amount(&id), 250);
}

#[test]
fn test_extend_stream_rejects_earlier_end() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    assert_eq!(
        s.contract.try_extend_stream(&id, &s.sender, &150),
        Err(Ok(Error::InvalidTimeRange))
    );
}

#[test]
fn test_extend_stream_rejects_non_sender() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    let stranger = Address::generate(&s.env);
    assert_eq!(
        s.contract.try_extend_stream(&id, &stranger, &300),
        Err(Ok(Error::Unauthorized))
    );
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

#[test]
fn test_cancel_after_partial_withdraw() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    // Recipient withdraws 250 at t=125, then the recipient cancels at t=150.
    set_time(&s.env, 125);
    assert_eq!(s.contract.withdraw(&id, &s.recipient), 250);

    set_time(&s.env, 150);
    s.contract.cancel(&id, &s.recipient);

    // Recipient now holds 500 total (250 + 250 vested-but-unwithdrawn).
    assert_eq!(s.token.balance(&s.recipient), 500);
    // Sender is refunded the unstreamed 500.
    assert_eq!(s.token.balance(&s.sender), 1_000_000 - 500);
}

#[test]
fn test_cancel_by_stranger_fails() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    set_time(&s.env, 150);
    let stranger = Address::generate(&s.env);
    let res = s.contract.try_cancel(&id, &stranger);
    assert_eq!(res, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_double_cancel_fails() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    set_time(&s.env, 150);
    s.contract.cancel(&id, &s.sender);
    let res = s.contract.try_cancel(&id, &s.sender);
    assert_eq!(res, Err(Ok(Error::AlreadyCancelled)));
}

#[test]
fn test_get_unknown_stream_fails() {
    let s = setup();
    let res = s.contract.try_get_stream(&42);
    assert_eq!(res, Err(Ok(Error::StreamNotFound)));
}

#[test]
fn test_withdraw_after_cancel_fails() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    set_time(&s.env, 150);
    s.contract.cancel(&id, &s.sender);

    set_time(&s.env, 180);
    let res = s.contract.try_withdraw(&id, &s.recipient);
    assert_eq!(res, Err(Ok(Error::AlreadyCancelled)));
}

#[test]
fn test_create_stream_requires_sender_auth() {
    let s = setup();
    s.contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);

    // The recorded authorization must come from the sender and target the
    // create_stream function with the exact arguments supplied.
    let auths = s.env.auths();
    let (who, invocation) = &auths[0];
    assert_eq!(who, &s.sender);
    assert_eq!(
        invocation.function,
        AuthorizedFunction::Contract((
            s.contract.address.clone(),
            Symbol::new(&s.env, "create_stream"),
            (
                s.sender.clone(),
                s.recipient.clone(),
                1_000_i128,
                100_u64,
                200_u64,
            )
                .into_val(&s.env),
        ))
    );
}

#[test]
fn test_multiple_streams_have_independent_ids() {
    let s = setup();
    let first = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);
    let second = s
        .contract
        .create_stream(&s.sender, &s.recipient, &2_000, &300, &400);

    assert_eq!(first, 0);
    assert_eq!(second, 1);
    assert_eq!(s.contract.stream_counter(), 2);
    assert_eq!(s.contract.get_stream(&second).total, 2_000);
    // Both escrows are held simultaneously by the contract.
    assert_eq!(s.token.balance(&s.contract.address), 3_000);
}

#[test]
fn test_create_stream_before_initialize_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(StreamPayContract, ());
    let contract = StreamPayContractClient::new(&env, &contract_id);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let res = contract.try_create_stream(&sender, &recipient, &1_000, &100, &200);
    assert_eq!(res, Err(Ok(Error::NotInitialized)));
}

#[test]
fn test_views_before_initialize_fail() {
    let env = Env::default();
    let contract_id = env.register(StreamPayContract, ());
    let contract = StreamPayContractClient::new(&env, &contract_id);

    assert_eq!(contract.try_get_admin(), Err(Ok(Error::NotInitialized)));
    assert_eq!(contract.try_get_token(), Err(Ok(Error::NotInitialized)));
}

// --- #47: constants module + centralized amount-limit validation -----------

#[test]
fn test_min_stream_amount_constant() {
    // The documented floor must be exactly 1 so dust (0) streams are rejected.
    assert_eq!(crate::MIN_STREAM_AMOUNT, 1);
}

#[test]
fn test_create_stream_accepts_minimum_amount() {
    let s = setup();
    // Exactly the minimum accepted amount (1) must succeed, not error.
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1, &100, &200);
    assert_eq!(id, 0);
    assert_eq!(s.contract.get_stream(&id).total, 1);
}

#[test]
fn test_create_stream_rejects_below_minimum() {
    let s = setup();
    // Zero is below MIN_STREAM_AMOUNT and must be rejected as InvalidAmount
    // (the central `is_valid_amount` guard).
    let res = s
        .contract
        .try_create_stream(&s.sender, &s.recipient, &0, &100, &200);
    assert_eq!(res, Err(Ok(Error::InvalidAmount)));
}

#[test]
fn test_create_stream_rejects_negative_amount() {
    let s = setup();
    // Negative amounts are non-positive and must be rejected identically.
    let res = s
        .contract
        .try_create_stream(&s.sender, &s.recipient, &-5, &100, &200);
    assert_eq!(res, Err(Ok(Error::InvalidAmount)));
}

#[test]
fn test_top_up_accepts_minimum_amount() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);
    // top_up also routes through the same amount validation; 1 is valid.
    assert_eq!(s.contract.top_up(&id, &s.sender, &1), 1_001);
}

#[test]
fn test_top_up_rejects_below_minimum() {
    let s = setup();
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &100, &200);
    assert_eq!(
        s.contract.try_top_up(&id, &s.sender, &0),
        Err(Ok(Error::InvalidAmount))
    );
}

#[test]
fn test_is_valid_amount_helper() {
    use crate::constants::is_valid_amount;
    // Only amounts >= MIN_STREAM_AMOUNT (1) are valid; 0 and negatives are not.
    assert!(is_valid_amount(1));
    assert!(is_valid_amount(1_000));
    assert!(!is_valid_amount(0));
    assert!(!is_valid_amount(-1));
}

// --- #44: input normalization helpers ---------------------------------------

#[test]
fn test_normalize_start_time_helper() {
    use crate::normalize::normalize_start_time;
    // The 0 sentinel resolves to the ledger clock; other values pass through.
    assert_eq!(normalize_start_time(1_000, 0), 1_000);
    assert_eq!(normalize_start_time(1_000, 500), 500);
    assert_eq!(normalize_start_time(1_000, 2_000), 2_000);
    assert_eq!(normalize_start_time(0, 0), 0);
}

#[test]
fn test_clamp_to_window_helper() {
    use crate::normalize::clamp_to_window;
    // Values are clamped into [start, end] and untouched inside it.
    assert_eq!(clamp_to_window(100, 200, 50), 100);
    assert_eq!(clamp_to_window(100, 200, 100), 100);
    assert_eq!(clamp_to_window(100, 200, 150), 150);
    assert_eq!(clamp_to_window(100, 200, 200), 200);
    assert_eq!(clamp_to_window(100, 200, 500), 200);
}

#[test]
fn test_create_stream_zero_start_begins_now() {
    let s = setup();
    // A start_time of 0 is normalized to the current ledger timestamp.
    set_time(&s.env, 1_000);
    let id = s
        .contract
        .create_stream(&s.sender, &s.recipient, &1_000, &0, &2_000);

    let stream = s.contract.get_stream(&id);
    assert_eq!(stream.start, 1_000);
    assert_eq!(stream.end, 2_000);

    // Vesting runs from "now": halfway through the window vests half.
    set_time(&s.env, 1_500);
    assert_eq!(s.contract.streamed_amount(&id), 500);
}

#[test]
fn test_create_stream_zero_start_rejects_end_before_now() {
    let s = setup();
    // With start normalized to now (1_000), an end at or before it is invalid.
    set_time(&s.env, 1_000);
    let res = s
        .contract
        .try_create_stream(&s.sender, &s.recipient, &1_000, &0, &500);
    assert_eq!(res, Err(Ok(Error::InvalidTimeRange)));
}
