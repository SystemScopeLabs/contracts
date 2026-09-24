//! Protocols spoken over ports (`docs/m0-design.md` §6).
//!
//! The set of messages is closed: every protocol is a variant of [`Message`], so the
//! runtime can check and canonically encode any message without dynamic typing.

pub mod block_v0;
pub mod irq_v0;
pub mod mem;
pub mod mem_v1;

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
    /// A `mem.v1` message.
    MemV1(mem_v1::MemMsg),
    /// An `irq.v0` message.
    Irq(irq_v0::IrqMsg),
    /// A `block.v0` message.
    Block(block_v0::BlockMsg),
}

impl Message {
    /// The protocol this message belongs to.
    pub fn protocol(&self) -> ProtocolId {
        match self {
            Message::Mem(_) => mem::PROTOCOL,
            Message::MemV1(_) => mem_v1::PROTOCOL,
            Message::Irq(_) => irq_v0::PROTOCOL,
            Message::Block(_) => block_v0::PROTOCOL,
        }
    }
}

impl From<mem::MemMsg> for Message {
    fn from(msg: mem::MemMsg) -> Message {
        Message::Mem(msg)
    }
}

impl From<mem_v1::MemMsg> for Message {
    fn from(msg: mem_v1::MemMsg) -> Message {
        Message::MemV1(msg)
    }
}

impl From<irq_v0::IrqMsg> for Message {
    fn from(msg: irq_v0::IrqMsg) -> Message {
        Message::Irq(msg)
    }
}

impl From<block_v0::BlockMsg> for Message {
    fn from(msg: block_v0::BlockMsg) -> Message {
        Message::Block(msg)
    }
}
