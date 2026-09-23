//! Trace records and their canonical stream encoding (`docs/m0-design.md` §8.1).
//!
//! The encoded stream is the only source of truth for a trace: its BLAKE3 hash is the
//! `TraceDigest`, and text exporters are views derived from the same records.

use crate::canonical::Encoder;
use crate::component::{ComponentId, PortId, PortSpec, Role};
use crate::event::EventKey;
use crate::time::{ClockDomain, Rounding};
use crate::topology::LinkLatency;

/// First bytes of every trace stream.
pub const TRACE_MAGIC: [u8; 8] = *b"SSTRACE\0";

/// Version of the trace stream layout. Any change to the layout, header, record
/// encoding, value tags, or `runtime.dispatch` fields must bump it.
pub const TRACE_FORMAT_VERSION: u32 = 2;

/// The value of the `contracts_version` field in trace headers and snapshots: the
/// [`COMPATIBILITY_ID`](crate::COMPATIBILITY_ID), under the name of that field. It is not
/// the crate's Cargo version.
pub const CONTRACTS_VERSION: &str = crate::COMPATIBILITY_ID;

/// Kind of the record the runtime emits for every dispatched event.
pub const DISPATCH_KIND: &str = "runtime.dispatch";

const RECORD_MARKER: u8 = 0x01;
const END_MARKER: u8 = 0x00;

/// A traced value. There are deliberately no floating-point values.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    /// Tag 0.
    U64(u64),
    /// Tag 1.
    I64(i64),
    /// Tag 2.
    Bool(bool),
    /// Tag 3.
    Str(String),
    /// Tag 4.
    Bytes(Vec<u8>),
}

impl Value {
    /// Writes the tag, then the payload.
    pub fn encode(&self, e: &mut Encoder) {
        match self {
            Value::U64(v) => {
                e.u8(0);
                e.u64(*v);
            }
            Value::I64(v) => {
                e.u8(1);
                e.i64(*v);
            }
            Value::Bool(v) => {
                e.u8(2);
                e.bool(*v);
            }
            Value::Str(v) => {
                e.u8(3);
                e.str(v);
            }
            Value::Bytes(v) => {
                e.u8(4);
                e.bytes(v);
            }
        }
    }
}

/// What was running when a record was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TraceAt {
    /// During `init`, before any event was dispatched.
    Init,
    /// While handling the event with this key.
    Event(EventKey),
}

/// Who made a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TraceOrigin {
    /// A component, through `ctx.trace()`.
    Component,
    /// The runtime itself, such as a `runtime.dispatch` record.
    Runtime,
}

/// One trace record.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TraceRecord {
    /// What was running.
    pub at: TraceAt,
    /// Who made it.
    pub origin: TraceOrigin,
    /// The emitting component; for runtime records, the event's target.
    pub component: ComponentId,
    /// What happened.
    pub kind: &'static str,
    /// Details, in the order given.
    pub fields: Vec<(&'static str, Value)>,
}

impl TraceRecord {
    /// Writes the record body, without its stream marker.
    pub fn encode(&self, e: &mut Encoder) {
        match self.at {
            TraceAt::Init => e.u8(0),
            TraceAt::Event(key) => {
                e.u8(1);
                e.u64(key.tick.0);
                e.u8(key.phase as u8);
                e.u64(key.sequence);
            }
        }
        e.u8(match self.origin {
            TraceOrigin::Component => 0,
            TraceOrigin::Runtime => 1,
        });
        e.u32(self.component.0);
        e.str(self.kind);
        e.len(self.fields.len());
        for (name, value) in &self.fields {
            e.str(name);
            value.encode(e);
        }
    }
}

/// A component as recorded in a trace header.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ComponentDecl {
    /// The component's path.
    pub path: String,
    /// The component's type name.
    pub type_name: &'static str,
    /// The component's ports, in `PortId` order.
    pub ports: Vec<PortSpec>,
}

/// A link as recorded in a trace header.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LinkDecl {
    /// One end.
    pub a: (ComponentId, PortId),
    /// The other end.
    pub b: (ComponentId, PortId),
    /// Added delay, if any.
    pub latency: Option<LinkLatency>,
}

/// Everything needed to interpret a trace's records.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TraceHeader {
    /// Session tick resolution.
    pub ticks_per_second: u64,
    /// Session seed.
    pub seed: u64,
    /// Version of the contracts the session ran with.
    pub contracts_version: String,
    /// BLAKE3 of [`encode_topology`] over `components` and `links`.
    pub topology_hash: [u8; 32],
    /// Clock domains, in id order.
    pub clock_domains: Vec<ClockDomain>,
    /// Components, in `ComponentId` order.
    pub components: Vec<ComponentDecl>,
    /// Links, in declaration order.
    pub links: Vec<LinkDecl>,
}

impl TraceHeader {
    /// Writes the header.
    pub fn encode(&self, e: &mut Encoder) {
        e.u64(self.ticks_per_second);
        e.u64(self.seed);
        e.str(&self.contracts_version);
        e.raw(&self.topology_hash);
        encode_clock_domains(e, &self.clock_domains);
        encode_topology(e, &self.components, &self.links);
    }
}

/// Writes clock domains as a sequence of
/// `id u32 · freq num u64 · freq den u64 · offset u64 · rounding u8`.
pub fn encode_clock_domains(e: &mut Encoder, domains: &[ClockDomain]) {
    e.len(domains.len());
    for d in domains {
        e.u32(d.id().0);
        e.u64(d.frequency().num());
        e.u64(d.frequency().den());
        e.u64(d.offset().0);
        e.u8(match d.edge_rounding() {
            Rounding::Floor => 0,
            Rounding::Ceil => 1,
        });
    }
}

/// Writes the structural topology: the component sequence, then the link sequence. The
/// `topology_hash` is BLAKE3 of exactly these bytes (§6).
pub fn encode_topology(e: &mut Encoder, components: &[ComponentDecl], links: &[LinkDecl]) {
    e.len(components.len());
    for c in components {
        e.str(&c.path);
        e.str(c.type_name);
        e.len(c.ports.len());
        for p in &c.ports {
            e.str(p.name);
            e.str(p.protocol.name);
            e.u16(p.protocol.version);
            e.u8(match p.role {
                Role::Initiator => 0,
                Role::Target => 1,
            });
        }
    }
    e.len(links.len());
    for l in links {
        for (component, port) in [l.a, l.b] {
            e.u32(component.0);
            e.u16(port.0);
        }
        match l.latency {
            None => e.u8(0),
            Some(LinkLatency::After(d)) => {
                e.u8(1);
                e.u128(d.as_femtoseconds());
            }
            Some(LinkLatency::Cycles { domain, k }) => {
                e.u8(2);
                e.u32(domain.0);
                e.u64(k);
            }
        }
    }
}

/// Encodes a complete trace stream: magic, version, header, marked records, and the
/// counted end marker.
pub fn encode_stream(header: &TraceHeader, records: &[TraceRecord]) -> Vec<u8> {
    let mut e = Encoder::new();
    e.raw(&TRACE_MAGIC);
    e.u32(TRACE_FORMAT_VERSION);
    header.encode(&mut e);
    for record in records {
        e.u8(RECORD_MARKER);
        record.encode(&mut e);
    }
    e.u8(END_MARKER);
    e.u64(records.len() as u64);
    e.into_bytes()
}
