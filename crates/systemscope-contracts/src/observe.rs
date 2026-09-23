//! Observation that cannot perturb the simulation (`docs/m0-design.md` §8.2).
//!
//! Observers see dispatched events, trace records, and a read-only [`WorldView`]. They get
//! no context, so they can neither draw from an RNG nor schedule events, and the view never
//! hands out a component, so they cannot change component state.

use crate::component::{Component, ComponentId, Delivered};
use crate::event::EventKey;
use crate::time::Tick;
use crate::trace::{TraceRecord, Value};

/// A component's state as named values, for observers and inspectors. Never read back by
/// the simulation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StateView {
    /// Named values, in the order the component chose.
    pub fields: Vec<(&'static str, Value)>,
}

impl StateView {
    /// The first value named `name`, if any.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.fields.iter().find(|(n, _)| *n == name).map(|(_, v)| v)
    }
}

/// Whether the driver should keep running after an observer callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// Keep going.
    Continue,
    /// Return control to the driver at this event boundary.
    Pause,
}

/// An event that was just dispatched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventView<'a> {
    /// The event's key.
    pub key: EventKey,
    /// The component that scheduled it.
    pub source: ComponentId,
    /// The component that handled it.
    pub target: ComponentId,
    /// What was delivered.
    pub delivery: &'a Delivered,
}

/// A read-only view of the simulated world.
///
/// It holds only shared references and exposes nothing but the time, component types,
/// and [`Component::inspect`]. No method returns a component, so no observer can call a
/// component's mutating methods through it.
pub struct WorldView<'a> {
    now: Tick,
    components: &'a [Box<dyn Component>],
}

impl<'a> WorldView<'a> {
    /// A view of `components` at `now`. Called by the runtime.
    pub fn new(now: Tick, components: &'a [Box<dyn Component>]) -> WorldView<'a> {
        WorldView { now, components }
    }

    /// The tick being observed.
    pub fn now(&self) -> Tick {
        self.now
    }

    /// Number of components; their ids are `0..count`.
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    /// The type name of component `id`.
    pub fn type_name(&self, id: ComponentId) -> Option<&'static str> {
        self.component(id).map(|c| c.type_name())
    }

    /// The state of component `id`.
    pub fn inspect(&self, id: ComponentId) -> Option<StateView> {
        self.component(id).map(|c| c.inspect())
    }

    fn component(&self, id: ComponentId) -> Option<&dyn Component> {
        self.components.get(id.0 as usize).map(|c| c.as_ref())
    }
}

/// Watches a run. Every method has a default that does nothing and continues.
pub trait Observer {
    /// Called after each event's handler returns, so it sees the state the event produced.
    fn on_after_dispatch(&mut self, ev: &EventView<'_>, world: &WorldView<'_>) -> Control {
        let _ = (ev, world);
        Control::Continue
    }

    /// Called at each due observe point, after every simulation event at or before `now`.
    fn on_observe(&mut self, now: Tick, world: &WorldView<'_>) -> Control {
        let _ = (now, world);
        Control::Continue
    }

    /// Called for every trace record the session emits, in emission order.
    fn on_trace(&mut self, rec: &TraceRecord) {
        let _ = rec;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{InitContext, PortSpec, SimContext};
    use crate::error::SimError;
    use crate::event::Phase;
    use crate::snapshot::{RestoreError, SnapshotReader, SnapshotWriter};

    struct Counter(u64);

    impl Component for Counter {
        fn type_name(&self) -> &'static str {
            "test.counter"
        }
        fn ports(&self) -> Vec<PortSpec> {
            Vec::new()
        }
        fn init(&mut self, _: &mut dyn InitContext) -> Result<(), SimError> {
            Ok(())
        }
        fn handle_event(&mut self, _: &Delivered, _: &mut dyn SimContext) -> Result<(), SimError> {
            self.0 += 1;
            Ok(())
        }
        fn snapshot_schema_version(&self) -> u32 {
            0
        }
        fn snapshot(&self, _: &mut SnapshotWriter) {}
        fn restore(&mut self, _: &mut SnapshotReader<'_>, _: u32) -> Result<(), RestoreError> {
            Ok(())
        }
        fn inspect(&self) -> StateView {
            StateView {
                fields: vec![("count", Value::U64(self.0))],
            }
        }
    }

    struct Silent;

    impl Component for Silent {
        fn type_name(&self) -> &'static str {
            "test.silent"
        }
        fn ports(&self) -> Vec<PortSpec> {
            Vec::new()
        }
        fn init(&mut self, _: &mut dyn InitContext) -> Result<(), SimError> {
            Ok(())
        }
        fn handle_event(&mut self, _: &Delivered, _: &mut dyn SimContext) -> Result<(), SimError> {
            Ok(())
        }
        fn snapshot_schema_version(&self) -> u32 {
            0
        }
        fn snapshot(&self, _: &mut SnapshotWriter) {}
        fn restore(&mut self, _: &mut SnapshotReader<'_>, _: u32) -> Result<(), RestoreError> {
            Ok(())
        }
    }

    #[test]
    fn the_world_view_reads_types_and_inspections_by_id() {
        let components: Vec<Box<dyn Component>> = vec![Box::new(Counter(7)), Box::new(Silent)];
        let world = WorldView::new(Tick(42), &components);
        assert_eq!(world.now(), Tick(42));
        assert_eq!(world.component_count(), 2);
        assert_eq!(world.type_name(ComponentId(0)), Some("test.counter"));
        assert_eq!(world.type_name(ComponentId(2)), None);
        let counter = world.inspect(ComponentId(0)).unwrap();
        assert_eq!(counter.get("count"), Some(&Value::U64(7)));
        assert_eq!(counter.get("missing"), None);
        // The default inspection shows nothing.
        assert_eq!(world.inspect(ComponentId(1)), Some(StateView::default()));
        assert_eq!(world.inspect(ComponentId(2)), None);
    }

    #[test]
    fn default_observer_callbacks_continue() {
        struct Idle;
        impl Observer for Idle {}
        let components: Vec<Box<dyn Component>> = Vec::new();
        let world = WorldView::new(Tick(0), &components);
        let delivery = Delivered::Wake { token: 1 };
        let ev = EventView {
            key: EventKey {
                tick: Tick(0),
                phase: Phase::Request,
                sequence: 0,
            },
            source: ComponentId(0),
            target: ComponentId(0),
            delivery: &delivery,
        };
        let mut idle = Idle;
        assert_eq!(idle.on_after_dispatch(&ev, &world), Control::Continue);
        assert_eq!(idle.on_observe(Tick(0), &world), Control::Continue);
        idle.on_trace(&TraceRecord {
            at: crate::trace::TraceAt::Init,
            origin: crate::trace::TraceOrigin::Component,
            component: ComponentId(0),
            kind: "k",
            fields: Vec::new(),
        });
    }
}
