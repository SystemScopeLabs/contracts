//! The component contract (`docs/m0-design.md` §5).
//!
//! A component cannot see other components and never handles physical ticks. Every
//! interaction happens through events whose time and source the runtime assigns.

use crate::error::SimError;
use crate::event::{Phase, ScheduleWhen};
use crate::observe::StateView;
use crate::protocol::{Message, ProtocolId};
use crate::rng::SimRng;
use crate::snapshot::{RestoreError, SnapshotReader, SnapshotWriter};
use crate::time::Tick;
use crate::trace::Value;

/// Identifies a component within a session. Assigned in topology declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentId(pub u32);

impl ComponentId {
    /// The source recorded for events the runtime schedules itself.
    pub const RUNTIME: ComponentId = ComponentId(u32::MAX);
}

/// Identifies one of a component's ports: its index in [`Component::ports`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PortId(pub u16);

/// Which side of a link a port is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// Starts transactions.
    Initiator,
    /// Serves transactions.
    Target,
}

/// Declares a port. A link must join an initiator and a target of the same protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PortSpec {
    /// Port name, unique within its component.
    pub name: &'static str,
    /// The protocol spoken on this port.
    pub protocol: ProtocolId,
    /// Initiator or target.
    pub role: Role,
}

/// What a component receives when one of its events runs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Delivered {
    /// A message arrived on one of the component's ports.
    Message {
        /// The receiving port.
        port: PortId,
        /// The message.
        msg: Message,
    },
    /// A wake-up the component scheduled for itself.
    Wake {
        /// The token passed to `wake_self`.
        token: u64,
    },
}

/// What a component may do during `init`, before anything has been dispatched.
///
/// Times are relative to tick 0. Errors are sticky: once a call fails, the runtime faults
/// even if the component ignores the error.
pub trait InitContext {
    /// The component being initialized or run.
    fn component(&self) -> ComponentId;

    /// Sends `msg` out of `port`. The runtime adds link latency and stamps the source.
    fn send(
        &mut self,
        port: PortId,
        msg: Message,
        when: ScheduleWhen,
        phase: Phase,
    ) -> Result<(), SimError>;

    /// Schedules a [`Delivered::Wake`] with `token` for this component.
    fn wake_self(&mut self, when: ScheduleWhen, phase: Phase, token: u64) -> Result<(), SimError>;

    /// This component's random stream, owned and snapshotted by the runtime.
    fn rng(&mut self) -> &mut dyn SimRng;

    /// Records a trace entry. Write-only: there is no way to learn whether tracing is on,
    /// and when it is off the record is simply dropped.
    fn trace(&mut self, kind: &'static str, fields: Vec<(&'static str, Value)>);
}

/// What a component may do while handling an event.
pub trait SimContext: InitContext {
    /// The tick of the event being handled. For diagnostics and traces only.
    fn now(&self) -> Tick;

    /// The phase of the event being handled.
    fn phase(&self) -> Phase;
}

/// A simulated component.
pub trait Component {
    /// A stable name for the component's type, recorded in the topology.
    fn type_name(&self) -> &'static str;

    /// The component's ports. [`PortId`] `i` is the `i`-th entry.
    fn ports(&self) -> Vec<PortSpec>;

    /// Called once for a new session, in `ComponentId` order. Never called on restore.
    fn init(&mut self, ctx: &mut dyn InitContext) -> Result<(), SimError>;

    /// Handles one delivered event.
    fn handle_event(&mut self, ev: &Delivered, ctx: &mut dyn SimContext) -> Result<(), SimError>;

    /// The layout version of [`Component::snapshot`]. Restore requires an exact match.
    fn snapshot_schema_version(&self) -> u32;

    /// Writes every piece of state that affects future behavior, canonically.
    fn snapshot(&self, w: &mut SnapshotWriter);

    /// Replaces this component's state with a snapshot written by the same schema. Called
    /// on a freshly elaborated component instead of `init`. The runtime rejects bytes left
    /// unread.
    fn restore(
        &mut self,
        r: &mut SnapshotReader<'_>,
        schema_version: u32,
    ) -> Result<(), RestoreError>;

    /// A read-only view of the component's state for observers and inspectors. Must not
    /// change the component; the default shows nothing.
    fn inspect(&self) -> StateView {
        StateView::default()
    }
}
