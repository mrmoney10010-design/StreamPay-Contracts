# Changelog

All notable changes to StreamPay are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `top_up` to escrow additional funds into an active stream.
- `extend_stream` to push back an active stream's end time and slow vesting.
- `duration`, `elapsed`, `percent_withdrawn`, `get_status`, and `is_active`
  view getters.
- `get_summary` view returning a `StreamSummary` snapshot in a single call.
- `MIN_STREAM_AMOUNT` guard rejecting dust streams on creation.
- `AmountBelowMinimum` and `StreamNotActive` error variants.
- `toppedup` and `extended` lifecycle events.
- `doc` Makefile target.

## [0.1.0]

### Added

- Initial release: linear payment streaming with create, withdraw, and cancel.
- Time-based vesting views: `streamed_amount`, `withdrawable_amount`,
  `remaining_amount`, and `progress_bps`.
- Events for stream creation, withdrawal, and cancellation.
- Checked arithmetic and authorization guards throughout.
