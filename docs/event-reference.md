# Event Reference

This page documents the contract events emitted by StreamPay.

Each event uses the stream id as a topic where applicable so indexers can group lifecycle updates without re-reading contract state. The payloads below match the event helpers in [src/events.rs](../src/events.rs).

| Event             | Topics                 | Data                                            |
| ----------------- | ---------------------- | ----------------------------------------------- |
| `created`         | `("created", id)`      | `(sender, recipient, total)`                    |
| `toppedup`        | `("toppedup", id)`     | `(sender, amount, new_total)`                   |
| `extended`        | `("extended", id)`     | `(sender, old_end, new_end)`                    |
| `withdrawn`       | `("withdrawn", id)`    | `(recipient, amount)`                           |
| `cancelled`       | `("cancelled", id)`    | `(caller, sender_refund, recipient_paid)`       |
| `admin_scheduled` | `("admin_scheduled",)` | `(current_admin, pending_admin, execute_after)` |
| `admin_transfer`  | `("admin_transfer",)`  | `(previous_admin, new_admin)`                   |
| `admin_cancelled` | `("admin_cancelled",)` | `admin`                                         |

The contract also emits token-transfer events through the underlying Stellar Asset Contract whenever escrowed funds move, but those are outside StreamPay's own event namespace.
