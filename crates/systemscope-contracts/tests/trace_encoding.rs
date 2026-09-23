//! Trace encoding: hand-written golden bytes and an independent decoder
//! (`docs/m0-design.md` §8.1).
//!
//! The golden vectors below are written byte by byte from the specification, not produced
//! by the encoder. The decoder is a separate implementation of the same layout; if every
//! encoded sequence decodes back to itself, no two sequences share an encoding.

use proptest::prelude::*;
use systemscope_contracts::COMPATIBILITY_ID;
use systemscope_contracts::component::{ComponentId, PortId, PortSpec, Role};
use systemscope_contracts::event::{EventKey, Phase};
use systemscope_contracts::protocol::ProtocolId;
use systemscope_contracts::time::{
    ClockDomain, ClockDomainId, Duration, Frequency, Rounding, SimulationClock, Tick,
};
use systemscope_contracts::topology::LinkLatency;
use systemscope_contracts::trace::{
    CONTRACTS_VERSION, ComponentDecl, LinkDecl, TRACE_FORMAT_VERSION, TraceAt, TraceHeader,
    TraceOrigin, TraceRecord, Value, encode_stream,
};

fn record_a() -> TraceRecord {
    TraceRecord {
        at: TraceAt::Event(EventKey {
            tick: Tick(0x1122),
            phase: Phase::Complete,
            sequence: 7,
        }),
        origin: TraceOrigin::Component,
        component: ComponentId(3),
        kind: "k",
        fields: vec![
            ("a", Value::U64(1)),
            ("b", Value::I64(-1)),
            ("c", Value::Bool(true)),
            ("d", Value::Str("xy".into())),
            ("e", Value::Bytes(vec![0xAA])),
        ],
    }
}

#[rustfmt::skip]
const RECORD_A: &[u8] = &[
    0x01,                                           // at: Event
    0x22, 0x11, 0, 0, 0, 0, 0, 0,                   //   tick
    0x02,                                           //   phase: Complete
    0x07, 0, 0, 0, 0, 0, 0, 0,                      //   sequence
    0x00,                                           // origin: Component
    0x03, 0, 0, 0,                                  // component
    0x01, 0, 0, 0, b'k',                            // kind
    0x05, 0, 0, 0,                                  // 5 fields
    0x01, 0, 0, 0, b'a', 0x00, 1, 0, 0, 0, 0, 0, 0, 0,                     // U64(1)
    0x01, 0, 0, 0, b'b', 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // I64(-1)
    0x01, 0, 0, 0, b'c', 0x02, 0x01,                                         // Bool(true)
    0x01, 0, 0, 0, b'd', 0x03, 0x02, 0, 0, 0, b'x', b'y',                    // Str("xy")
    0x01, 0, 0, 0, b'e', 0x04, 0x01, 0, 0, 0, 0xAA,                          // Bytes([AA])
];

fn record_b() -> TraceRecord {
    TraceRecord {
        at: TraceAt::Init,
        origin: TraceOrigin::Runtime,
        component: ComponentId(0),
        kind: "",
        fields: Vec::new(),
    }
}

#[rustfmt::skip]
const RECORD_B: &[u8] = &[
    0x00,          // at: Init
    0x01,          // origin: Runtime
    0, 0, 0, 0,    // component
    0, 0, 0, 0,    // empty kind
    0, 0, 0, 0,    // no fields
];

fn header() -> TraceHeader {
    let clock = SimulationClock::new(1000).unwrap();
    let domain = ClockDomain::new(
        &clock,
        ClockDomainId(0),
        Frequency::new(10, 1).unwrap(),
        Tick(9),
        Rounding::Ceil,
    )
    .unwrap();
    TraceHeader {
        ticks_per_second: 1000,
        seed: 5,
        contracts_version: "v".into(),
        topology_hash: core::array::from_fn(|i| i as u8),
        clock_domains: vec![domain],
        components: vec![ComponentDecl {
            path: "c".into(),
            type_name: "t",
            ports: vec![PortSpec {
                name: "p",
                protocol: ProtocolId {
                    name: "m",
                    version: 2,
                },
                role: Role::Target,
            }],
        }],
        links: vec![
            LinkDecl {
                a: (ComponentId(0), PortId(0)),
                b: (ComponentId(0), PortId(0)),
                latency: Some(LinkLatency::After(Duration::from_fs(3))),
            },
            LinkDecl {
                a: (ComponentId(0), PortId(0)),
                b: (ComponentId(0), PortId(0)),
                latency: Some(LinkLatency::Cycles {
                    domain: ClockDomainId(1),
                    k: 4,
                }),
            },
        ],
    }
}

#[rustfmt::skip]
const HEADER: &[u8] = &[
    0xE8, 0x03, 0, 0, 0, 0, 0, 0,          // ticks_per_second = 1000
    0x05, 0, 0, 0, 0, 0, 0, 0,             // seed
    0x01, 0, 0, 0, b'v',                   // contracts_version
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, // topology_hash, 32 raw bytes
    0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
    0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F,
    0x01, 0, 0, 0,                         // 1 clock domain
    0, 0, 0, 0,                            //   id
    0x0A, 0, 0, 0, 0, 0, 0, 0,             //   freq num
    0x01, 0, 0, 0, 0, 0, 0, 0,             //   freq den
    0x09, 0, 0, 0, 0, 0, 0, 0,             //   offset
    0x01,                                  //   rounding: Ceil
    0x01, 0, 0, 0,                         // 1 component
    0x01, 0, 0, 0, b'c',                   //   path
    0x01, 0, 0, 0, b't',                   //   type_name
    0x01, 0, 0, 0,                         //   1 port
    0x01, 0, 0, 0, b'p',                   //     name
    0x01, 0, 0, 0, b'm',                   //     protocol name
    0x02, 0,                               //     protocol version
    0x01,                                  //     role: Target
    0x02, 0, 0, 0,                         // 2 links
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,    //   a, b
    0x01,                                  //   latency: After
    0x03, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // femtoseconds u128
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,    //   a, b
    0x02,                                  //   latency: Cycles
    0x01, 0, 0, 0,                         //   domain
    0x04, 0, 0, 0, 0, 0, 0, 0,             //   k
];

fn golden_stream(records: &[&[u8]]) -> Vec<u8> {
    let mut out = b"SSTRACE\0".to_vec();
    out.extend_from_slice(&[0x02, 0, 0, 0]); // format_version 2
    out.extend_from_slice(HEADER);
    for r in records {
        out.push(0x01); // record marker
        out.extend_from_slice(r);
    }
    out.push(0x00); // end marker
    out.extend_from_slice(&(records.len() as u64).to_le_bytes());
    out
}

#[test]
fn format_version_is_two() {
    assert_eq!(TRACE_FORMAT_VERSION, 2);
}

/// The compatibility id is serialized into every snapshot and trace, so it is pinned here.
/// Changing it re-blesses every golden file (`docs/m1-design.md` §4.5). It is a constant,
/// not the crate's Cargo version, which may change without touching any digest.
#[test]
fn compatibility_id_is_pinned_and_is_what_headers_record() {
    assert_eq!(COMPATIBILITY_ID, "0.0.0");
    assert_eq!(CONTRACTS_VERSION, COMPATIBILITY_ID);
}

#[test]
fn stream_matches_hand_written_golden_bytes() {
    let stream = encode_stream(&header(), &[record_a(), record_b()]);
    assert_eq!(stream, golden_stream(&[RECORD_A, RECORD_B]));
}

#[test]
fn empty_stream_matches_golden_bytes() {
    assert_eq!(encode_stream(&header(), &[]), golden_stream(&[]));
}

#[test]
fn record_order_is_significant() {
    let ab = encode_stream(&header(), &[record_a(), record_b()]);
    let ba = encode_stream(&header(), &[record_b(), record_a()]);
    assert_ne!(ab, ba);
    assert_eq!(ba, golden_stream(&[RECORD_B, RECORD_A]));
}

#[test]
fn field_order_type_tags_and_lengths_are_significant() {
    let base = record_a();
    let mut swapped = base.clone();
    swapped.fields.swap(0, 1);
    let mut retagged = base.clone();
    retagged.fields[0].1 = Value::I64(1);
    let mut as_bytes = base.clone();
    as_bytes.fields[3].1 = Value::Bytes(b"xy".to_vec());
    let mut longer = base.clone();
    longer.fields[3].1 = Value::Str("xy\0".into());
    let encodings: Vec<Vec<u8>> = [base, swapped, retagged, as_bytes, longer]
        .iter()
        .map(|r| encode_stream(&header(), std::slice::from_ref(r)))
        .collect();
    for (i, a) in encodings.iter().enumerate() {
        for b in &encodings[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

// ---------------------------------------------------------------------------------------
// Independent decoder, used only to prove the encoding is unambiguous.

#[derive(Debug, PartialEq, Eq)]
enum Val {
    U64(u64),
    I64(i64),
    Bool(bool),
    Str(String),
    Bytes(Vec<u8>),
}

#[derive(Debug, PartialEq, Eq)]
struct Rec {
    at: Option<(u64, u8, u64)>,
    origin: u8,
    component: u32,
    kind: String,
    fields: Vec<(String, Val)>,
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take(&mut self, n: usize) -> &[u8] {
        assert!(self.0.len() >= n, "truncated stream");
        let (head, tail) = self.0.split_at(n);
        self.0 = tail;
        head
    }
    fn u8(&mut self) -> u8 {
        self.take(1)[0]
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.take(2).try_into().unwrap())
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take(8).try_into().unwrap())
    }
    fn bytes(&mut self) -> Vec<u8> {
        let n = self.u32() as usize;
        self.take(n).to_vec()
    }
    fn string(&mut self) -> String {
        String::from_utf8(self.bytes()).expect("valid UTF-8")
    }
}

fn skip_header(r: &mut Reader) {
    r.u64();
    r.u64();
    r.string();
    r.take(32);
    for _ in 0..r.u32() {
        r.take(4 + 8 + 8 + 8 + 1);
    }
    for _ in 0..r.u32() {
        r.string();
        r.string();
        for _ in 0..r.u32() {
            r.string();
            r.string();
            r.u16();
            r.u8();
        }
    }
    for _ in 0..r.u32() {
        r.take(12);
        match r.u8() {
            0 => {}
            1 => {
                r.take(16);
            }
            2 => {
                r.take(12);
            }
            t => panic!("bad latency tag {t}"),
        }
    }
}

fn decode(stream: &[u8]) -> Vec<Rec> {
    let mut r = Reader(stream);
    assert_eq!(r.take(8), b"SSTRACE\0");
    assert_eq!(r.u32(), 2);
    skip_header(&mut r);
    let mut out = Vec::new();
    loop {
        match r.u8() {
            0x00 => break,
            0x01 => {}
            m => panic!("bad record marker {m}"),
        }
        let at = match r.u8() {
            0 => None,
            1 => Some((r.u64(), r.u8(), r.u64())),
            t => panic!("bad at tag {t}"),
        };
        let origin = r.u8();
        let component = r.u32();
        let kind = r.string();
        let fields = (0..r.u32())
            .map(|_| {
                let name = r.string();
                let value = match r.u8() {
                    0 => Val::U64(r.u64()),
                    1 => Val::I64(r.u64() as i64),
                    2 => Val::Bool(match r.u8() {
                        0 => false,
                        1 => true,
                        b => panic!("bad bool {b}"),
                    }),
                    3 => Val::Str(r.string()),
                    4 => Val::Bytes(r.bytes()),
                    t => panic!("bad value tag {t}"),
                };
                (name, value)
            })
            .collect();
        out.push(Rec {
            at,
            origin,
            component,
            kind,
            fields,
        });
    }
    assert_eq!(r.u64(), out.len() as u64, "trailer count");
    assert!(r.0.is_empty(), "trailing bytes");
    out
}

fn expected(record: &TraceRecord) -> Rec {
    Rec {
        at: match record.at {
            TraceAt::Init => None,
            TraceAt::Event(k) => Some((k.tick.0, k.phase as u8, k.sequence)),
        },
        origin: match record.origin {
            TraceOrigin::Component => 0,
            TraceOrigin::Runtime => 1,
        },
        component: record.component.0,
        kind: record.kind.to_owned(),
        fields: record
            .fields
            .iter()
            .map(|(n, v)| {
                let v = match v {
                    Value::U64(x) => Val::U64(*x),
                    Value::I64(x) => Val::I64(*x),
                    Value::Bool(x) => Val::Bool(*x),
                    Value::Str(x) => Val::Str(x.clone()),
                    Value::Bytes(x) => Val::Bytes(x.clone()),
                };
                ((*n).to_owned(), v)
            })
            .collect(),
    }
}

#[test]
fn decoder_reads_the_golden_stream() {
    let stream = golden_stream(&[RECORD_A, RECORD_B]);
    assert_eq!(
        decode(&stream),
        [expected(&record_a()), expected(&record_b())]
    );
}

const KINDS: [&str; 4] = ["", "a", "ab", "toy.cpu.issue"];
const NAMES: [&str; 4] = ["", "x", "xy", "txn"];

fn value() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<u64>().prop_map(Value::U64),
        any::<i64>().prop_map(Value::I64),
        any::<bool>().prop_map(Value::Bool),
        "[a-z\u{e9}\0]{0,6}".prop_map(Value::Str),
        prop::collection::vec(any::<u8>(), 0..6).prop_map(Value::Bytes),
    ]
}

fn record() -> impl Strategy<Value = TraceRecord> {
    let at = prop_oneof![
        Just(TraceAt::Init),
        (any::<u64>(), 0usize..5, any::<u64>()).prop_map(|(t, p, s)| TraceAt::Event(EventKey {
            tick: Tick(t),
            phase: Phase::ALL[p],
            sequence: s,
        })),
    ];
    let origin = prop_oneof![Just(TraceOrigin::Component), Just(TraceOrigin::Runtime)];
    let fields = prop::collection::vec((prop::sample::select(&NAMES[..]), value()), 0..4);
    (
        at,
        origin,
        any::<u32>(),
        prop::sample::select(&KINDS[..]),
        fields,
    )
        .prop_map(|(at, origin, component, kind, fields)| TraceRecord {
            at,
            origin,
            component: ComponentId(component),
            kind,
            fields,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn every_record_sequence_decodes_back_to_itself(
        records in prop::collection::vec(record(), 0..6),
    ) {
        let decoded = decode(&encode_stream(&header(), &records));
        let wanted: Vec<Rec> = records.iter().map(expected).collect();
        prop_assert_eq!(decoded, wanted);
    }

    #[test]
    fn distinct_sequences_have_distinct_streams(
        a in prop::collection::vec(record(), 0..4),
        b in prop::collection::vec(record(), 0..4),
    ) {
        prop_assume!(a != b);
        prop_assert_ne!(encode_stream(&header(), &a), encode_stream(&header(), &b));
    }
}
