# ADR 0002: Compile as a cdylib for wasm32

- Status: Accepted
- Deciders: arisu6804

## Context

The StreamPay smart contract needs a clear, documented approach to "compile as a cdylib for wasm32" so the codebase stays consistent and auditable.

## Decision

We compile as a cdylib for wasm32 as the standard for this contract, in line with Soroban best practices.

## Consequences

Improves clarity, testability, and maintainability, and gives future contributors a recorded rationale to build on.
