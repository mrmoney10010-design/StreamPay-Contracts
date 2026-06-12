//! Linear vesting math for the StreamPay contract.
//!
//! Given a stream's total amount and time window, [`vested`] computes how much
//! has vested at a particular ledger timestamp:
//!
//! * `0` before `start`
//! * `total` at or after `end`
//! * a linear interpolation in between
//!
//! All arithmetic is checked; on overflow the functions return [`Error::Overflow`].

use crate::error::Error;
use crate::types::Stream;

/// Computes the linearly vested amount of `stream` at timestamp `now`.
pub fn vested(stream: &Stream, now: u64) -> Result<i128, Error> {
    if now <= stream.start {
        return Ok(0);
    }
    if now >= stream.end {
        return Ok(stream.total);
    }

    let elapsed = (now - stream.start) as i128;
    let duration = (stream.end - stream.start) as i128;

    let numerator = stream
        .total
        .checked_mul(elapsed)
        .ok_or(Error::Overflow)?;
    let result = numerator.checked_div(duration).ok_or(Error::Overflow)?;
    Ok(result)
}
