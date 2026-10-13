//! Trace comparison: the first divergence, with its tick and field.
//!
//! `compare-process-captures` drives this later; for now the CLI is what CI
//! runs. The comparison order is: header identity (version, scenario, map,
//! asset mode, seed, fix rows), then tick records in order, then the record
//! tail when one trace simply ran longer. Header identity first because two
//! traces that name different scenarios or asset modes are not comparable
//! runs; past that, the first observable difference is what the report names.
//!
//! Output is one line. `unchanged` when the traces agree, otherwise
//! `diverge tick=<n|header> field=<path> left=<v> right=<v>`. Field paths
//! index lists by position (`unit[j]` is the j-th unit of the tick's id-sorted
//! list), which stays unambiguous even when the lists differ.

use std::fmt;

use crate::trace::{TickRecord, Trace};

/// The first difference between two traces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    /// The tick the difference is on, `None` for header differences.
    pub tick: Option<u64>,
    /// Field path within that scope, for example `asset-mode` or `unit[0].x`.
    pub field: String,
    pub left: String,
    pub right: String,
}

impl fmt::Display for Divergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tick = match self.tick {
            Some(t) => t.to_string(),
            None => "header".to_string(),
        };
        write!(
            f,
            "diverge tick={tick} field={} left={} right={}",
            self.field, self.left, self.right
        )
    }
}

/// Reports the first difference between `left` and `right`, `None` when they agree.
pub fn first_divergence(left: &Trace, right: &Trace) -> Option<Divergence> {
    if let Some(d) = compare_header(left, right) {
        return Some(d);
    }
    let common = left.ticks.len().min(right.ticks.len());
    for i in 0..common {
        if let Some(d) = compare_tick(i as u64, &left.ticks[i], &right.ticks[i]) {
            return Some(d);
        }
    }
    if left.ticks.len() != right.ticks.len() {
        let tick = common as u64;
        let (l, r) = if left.ticks.len() > common {
            ("present", "absent")
        } else {
            ("absent", "present")
        };
        return Some(Divergence {
            tick: Some(tick),
            field: "record".to_string(),
            left: l.to_string(),
            right: r.to_string(),
        });
    }
    None
}

/// Compares one field pair and returns from the enclosing comparison function
/// on the first difference.
macro_rules! check {
    ($tick:expr, $field:expr, $left:expr, $right:expr) => {{
        let (left, right) = ($left, $right);
        if left != right {
            return Some(Divergence {
                tick: $tick,
                field: $field.to_string(),
                left,
                right,
            });
        }
    }};
}

fn compare_header(left: &Trace, right: &Trace) -> Option<Divergence> {
    let (l, r) = (&left.header, &right.header);
    check!(
        None,
        "version",
        l.format_version.to_string(),
        r.format_version.to_string()
    );
    check!(None, "scenario", l.scenario.clone(), r.scenario.clone());
    check!(None, "map", l.map.clone(), r.map.clone());
    check!(
        None,
        "asset-mode",
        l.asset_mode.as_str().to_string(),
        r.asset_mode.as_str().to_string()
    );
    check!(None, "seed", l.seed.to_string(), r.seed.to_string());
    let common = l.fix_rows.len().min(r.fix_rows.len());
    for i in 0..common {
        check!(
            None,
            format!("fix[{i}]"),
            format!(
                "{}.{}",
                l.fix_rows[i].disposition.as_str(),
                l.fix_rows[i].id
            ),
            format!(
                "{}.{}",
                r.fix_rows[i].disposition.as_str(),
                r.fix_rows[i].id
            )
        );
    }
    if l.fix_rows.len() != r.fix_rows.len() {
        let i = common;
        // A configuration row is named in full: "one side has no fix[0]" is
        // useless without knowing which row the other side activated.
        let extra = if l.fix_rows.len() > common {
            &l.fix_rows[i]
        } else {
            &r.fix_rows[i]
        };
        let token = format!("{}.{}", extra.disposition.as_str(), extra.id);
        let (left, right) = if l.fix_rows.len() > common {
            (token.as_str(), "absent")
        } else {
            ("absent", token.as_str())
        };
        return Some(Divergence {
            tick: None,
            field: format!("fix[{i}]"),
            left: left.to_string(),
            right: right.to_string(),
        });
    }
    None
}

fn compare_tick(tick: u64, left: &TickRecord, right: &TickRecord) -> Option<Divergence> {
    check!(
        Some(tick),
        "tick",
        left.tick.to_string(),
        right.tick.to_string()
    );

    let mut common = left.units.len().min(right.units.len());
    for i in 0..common {
        let (a, b) = (&left.units[i], &right.units[i]);
        check!(
            Some(tick),
            format!("unit[{i}].id"),
            a.id.to_string(),
            b.id.to_string()
        );
        check!(
            Some(tick),
            format!("unit[{i}].owner"),
            a.owner.to_string(),
            b.owner.to_string()
        );
        check!(
            Some(tick),
            format!("unit[{i}].x"),
            a.x.to_string(),
            b.x.to_string()
        );
        check!(
            Some(tick),
            format!("unit[{i}].y"),
            a.y.to_string(),
            b.y.to_string()
        );
        check!(
            Some(tick),
            format!("unit[{i}].target"),
            fmt_target(&a.target),
            fmt_target(&b.target)
        );
    }
    if let Some(d) = list_tail(tick, "unit", left.units.len(), right.units.len(), common) {
        return Some(d);
    }

    common = left.rng_draws.len().min(right.rng_draws.len());
    for i in 0..common {
        let (a, b) = (&left.rng_draws[i], &right.rng_draws[i]);
        check!(
            Some(tick),
            format!("rng[{i}].index"),
            a.index.to_string(),
            b.index.to_string()
        );
        check!(
            Some(tick),
            format!("rng[{i}].value"),
            format!("{:#018x}", a.value),
            format!("{:#018x}", b.value)
        );
    }
    if let Some(d) = list_tail(
        tick,
        "rng",
        left.rng_draws.len(),
        right.rng_draws.len(),
        common,
    ) {
        return Some(d);
    }

    common = left.timers.len().min(right.timers.len());
    for i in 0..common {
        let (a, b) = (&left.timers[i], &right.timers[i]);
        check!(
            Some(tick),
            format!("timer[{i}].seq"),
            a.seq.to_string(),
            b.seq.to_string()
        );
        check!(
            Some(tick),
            format!("timer[{i}].kind"),
            a.kind.as_str().to_string(),
            b.kind.as_str().to_string()
        );
        check!(
            Some(tick),
            format!("timer[{i}].name"),
            a.name.clone(),
            b.name.clone()
        );
    }
    if let Some(d) = list_tail(tick, "timer", left.timers.len(), right.timers.len(), common) {
        return Some(d);
    }
    None
}

/// Reports the first element one side has and the other lacks, if any.
fn list_tail(
    tick: u64,
    kind: &str,
    left_len: usize,
    right_len: usize,
    common: usize,
) -> Option<Divergence> {
    if left_len == right_len {
        return None;
    }
    let (left, right) = if left_len > common {
        ("present", "absent")
    } else {
        ("absent", "present")
    };
    Some(Divergence {
        tick: Some(tick),
        field: format!("{kind}[{common}]"),
        left: left.to_string(),
        right: right.to_string(),
    })
}

fn fmt_target(target: &Option<(i32, i32)>) -> String {
    match target {
        Some((x, y)) => format!("{x},{y}"),
        None => "absent".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::run;
    use crate::scenario::{smoke, smoke_config};
    use crate::trace::{AssetMode, FixDisposition, FixRow, Trace, TraceHeader};

    fn diverges(left: &Trace, right: &Trace) -> Divergence {
        first_divergence(left, right).expect("a divergence was expected")
    }

    #[test]
    fn identical_traces_are_unchanged() {
        let a = run(&smoke(), &smoke_config()).unwrap();
        let b = run(&smoke(), &smoke_config()).unwrap();
        assert_eq!(first_divergence(&a, &b), None);
    }

    #[test]
    fn a_different_asset_mode_is_reported_from_the_header() {
        let a = run(&smoke(), &smoke_config()).unwrap();
        let config = crate::scenario::RunConfig {
            asset_mode: AssetMode::Reforged,
            ..smoke_config()
        };
        let b = run(&smoke(), &config).unwrap();
        let divergence = diverges(&a, &b);
        assert_eq!(divergence.tick, None);
        assert_eq!(divergence.field, "asset-mode");
        assert_eq!(divergence.left, "vanilla");
        assert_eq!(divergence.right, "reforged");
        assert_ne!(
            a.canonical_hash(),
            b.canonical_hash(),
            "a vanilla run must never hash like a reforged one"
        );
    }

    #[test]
    fn a_fix_ledger_difference_is_reported() {
        let a = run(&smoke(), &smoke_config()).unwrap();
        let config = crate::scenario::RunConfig {
            fix_rows: vec![FixRow {
                id: "fix.armor-spread".to_string(),
                disposition: FixDisposition::Adopt,
            }],
            ..smoke_config()
        };
        let b = run(&smoke(), &config).unwrap();
        let divergence = diverges(&a, &b);
        assert_eq!(divergence.tick, None);
        assert_eq!(divergence.field, "fix[0]");
        assert_eq!(divergence.left, "absent");
        assert_eq!(divergence.right, "adopt.fix.armor-spread");
    }

    #[test]
    fn a_longer_run_reports_the_first_missing_record() {
        let mut scenario = smoke();
        scenario.ticks = 50;
        let short = run(&scenario, &smoke_config()).unwrap();
        let long = run(&smoke(), &smoke_config()).unwrap();
        let divergence = diverges(&short, &long);
        assert_eq!(divergence.tick, Some(50));
        assert_eq!(divergence.field, "record");
        assert_eq!(divergence.left, "absent");
        assert_eq!(divergence.right, "present");
    }

    #[test]
    fn the_report_is_one_line() {
        let a = run(&smoke(), &smoke_config()).unwrap();
        let mut b = a.clone();
        b.header = TraceHeader {
            asset_mode: AssetMode::Hd,
            ..a.header.clone()
        };
        let line = diverges(&a, &b).to_string();
        assert_eq!(
            line,
            "diverge tick=header field=asset-mode left=vanilla right=hd"
        );
    }
}
