//! Protocols spoken over ports (`docs/m0-design.md` §6).
//!
//! The set of messages is closed: every protocol is a variant of [`Message`], so the
//! runtime can check and canonically encode any message without dynamic typing.

pub mod mem;

/// Names a protocol and its version. Linked ports must agree on both.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProtocolId {
    /// Protocol name, such as `"mem"`.
    pub name: &'static str,
    /// Protocol version; any change to message layout requires a new version.
    pub version: u16,
}

/// A message of any protocol.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Message {
    /// A `mem.v0` message.
    Mem(mem::MemMsg),
}

impl Message {
    /// The protocol this message belongs to.
    pub fn protocol(&self) -> ProtocolId {
        match self {
            Message::Mem(_) => mem::PROTOCOL,
        }
    }
}

impl From<mem::MemMsg> for Message {
    fn from(msg: mem::MemMsg) -> Message {
        Message::Mem(msg)
    }
}
