//! Per-span allocation attribution for the render CPU workload. With `profile-cpu`
//! (Bevy `trace`), every system runs inside a span; this layer charges each span
//! the allocation calls and bytes its own thread made inside it, excluding child
//! spans. Elapsed time is recorded beside them as a report-only figure.
use crate::counting_allocator::{Totals, thread_totals};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
use tracing::{
    Subscriber,
    field::{Field, Visit},
    span,
};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

/// Spans are charged only while a measured phase runs.
static ACTIVE: AtomicBool = AtomicBool::new(false);
static STATS: Mutex<Option<HashMap<String, SpanStats>>> = Mutex::new(None);
/// Open spans on one thread need no allocation after this capacity is reserved.
const OPEN_SPAN_CAPACITY: usize = 256;

/// Counters of one span name, excluding child spans.
#[derive(Default, Clone, serde::Serialize)]
pub(super) struct SpanStats {
    pub(super) calls: u64,
    pub(super) allocation_calls: u64,
    pub(super) reallocation_calls: u64,
    pub(super) requested_bytes: u64,
    pub(super) elapsed_ns_report_only: u64,
}

struct Open {
    key: String,
    start: Totals,
    started: Instant,
    /// Allocations and time spent in child spans and this layer's own bookkeeping.
    child: Totals,
    child_ns: u64,
}

thread_local! {
    static OPEN: RefCell<Vec<Open>> = RefCell::new(Vec::with_capacity(OPEN_SPAN_CAPACITY));
}

struct Key(String);

#[derive(Default)]
struct NameField(Option<String>);

impl Visit for NameField {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "name" {
            self.0 = Some(value.to_owned());
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "name" && self.0.is_none() {
            self.0 = Some(format!("{value:?}"));
        }
    }
}

pub(super) struct SpanCharge;

pub(super) fn set_active(active: bool) {
    ACTIVE.store(active, Ordering::SeqCst);
}

/// Takes the exclusive stats gathered since the previous call.
pub(super) fn take() -> HashMap<String, SpanStats> {
    STATS.lock().unwrap().take().unwrap_or_default()
}

fn sub(a: Totals, b: Totals) -> Totals {
    a - b
}

fn add(a: Totals, b: Totals) -> Totals {
    Totals {
        calls: a.calls + b.calls,
        reallocations: a.reallocations + b.reallocations,
        bytes: a.bytes + b.bytes,
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for SpanCharge {
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let mut name = NameField::default();
        attrs.record(&mut name);
        let key = match name.0 {
            Some(name) => format!("{}: {name}", attrs.metadata().name()),
            None => attrs.metadata().name().to_owned(),
        };
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(Key(key));
        }
    }

    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        if !ACTIVE.load(Ordering::Relaxed) {
            return;
        }
        let Some(span) = ctx.span(id) else { return };
        let key = span
            .extensions()
            .get::<Key>()
            .map_or_else(String::new, |key| key.0.clone());
        OPEN.with(|open| {
            // Snapshots come last so this bookkeeping is not charged to the span.
            let mut open = open.borrow_mut();
            open.push(Open {
                key,
                start: Totals::default(),
                started: Instant::now(),
                child: Totals::default(),
                child_ns: 0,
            });
            let last = open.last_mut().expect("just pushed");
            last.start = thread_totals();
            last.started = Instant::now();
        });
    }

    fn on_exit(&self, _id: &span::Id, _ctx: Context<'_, S>) {
        if !ACTIVE.load(Ordering::Relaxed) {
            return;
        }
        let end = thread_totals();
        let ended = Instant::now();
        OPEN.with(|open| {
            let mut open = open.borrow_mut();
            let Some(frame) = open.pop() else { return };
            let inclusive = sub(end, frame.start);
            let inclusive_ns = ended.duration_since(frame.started).as_nanos() as u64;
            let own = sub(inclusive, frame.child);
            {
                let mut stats = STATS.lock().unwrap();
                let entry = stats
                    .get_or_insert_with(HashMap::new)
                    .entry(frame.key)
                    .or_default();
                entry.calls += 1;
                entry.allocation_calls += own.calls;
                entry.reallocation_calls += own.reallocations;
                entry.requested_bytes += own.bytes;
                entry.elapsed_ns_report_only += inclusive_ns.saturating_sub(frame.child_ns);
            }
            // The parent sees this span, and the bookkeeping above, as a child.
            if let Some(parent) = open.last_mut() {
                let after = thread_totals();
                parent.child = add(parent.child, sub(after, frame.start));
                parent.child_ns += Instant::now().duration_since(frame.started).as_nanos() as u64;
            }
        });
    }
}
