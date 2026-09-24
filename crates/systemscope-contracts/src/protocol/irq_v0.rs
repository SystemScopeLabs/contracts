//! `irq.v0`: a level-sensitive interrupt line (`docs/m2-design.md` §7.1).
//!
//! A source port (an `Initiator`) tells a sink port (a `Target`) the current level of its
//! output. The protocol carries the level and nothing else: there is no response, no
//! acknowledgement, no edge, and no source id or priority. What a level means belongs
//! to the components on each side, not to this protocol.
//!
//! # State is the level
//!
//! A sink keeps the last level it received on each port. A [`IrqMsg::Level`] equal to
//! that level changes nothing and is not an error, so the message is idempotent. Sources
//! send only on a change, but sinks must not depend on that. Every line is deasserted at
//! reset.
//!
//! # Timing is not part of the wire format
//!
//! The encoding carries no phase, cycle, or delivery rule. When a level is sent and when
//! a sink may act on it are rules of the components and topology that use this protocol
//! (`docs/m2-design.md` §7.1).
//!
//! # Versioning
//!
//! Variant order, field order, and the encoding of `asserted` are part of the canonical
//! encoding. Any change to them, including a new variant, is a new protocol version,
//! never an edit to this one.

use super::ProtocolId;

/// The `irq.v0` protocol.
pub const PROTOCOL: ProtocolId = ProtocolId {
    name: "irq",
    version: 0,
};

/// An `irq.v0` message. Variant and field order are part of the canonical encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IrqMsg {
    /// The sender's output level is now `asserted`. Sent from source to sink; there is no
    /// response.
    Level {
        /// `true` if the line is asserted.
        asserted: bool,
    },
}
