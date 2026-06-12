#![no_std]
//! StreamPay: a real-time payment-streaming smart contract for Stellar.
//!
//! StreamPay lets a sender escrow a fixed amount of a token and stream it
//! linearly to a recipient over a time window. The recipient can withdraw the
//! vested portion at any time, and either party can cancel an active stream.

use soroban_sdk::contract;

/// The StreamPay contract type.
#[contract]
pub struct StreamPayContract;
