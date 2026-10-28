//! Fixed-work budgets for the render workload. A budget file lists one upper bound per
//! counter; a run fails when any counter exceeds its bound or when the counter set and the
//! file disagree. Bounds only ever go down: see `docs/performance/render-cpu.md`.
use std::collections::BTreeMap;

/// Format of the budget file; bump when the key scheme changes.
const BUDGET_VERSION: u32 = 1;

#[derive(serde::Deserialize)]
struct Budget {
    version: u32,
    workload: String,
    limits: BTreeMap<String, u64>,
}

/// Compares `observed` counters with the budget text. Returns every violation.
pub(super) fn check(
    workload: &str,
    observed: &BTreeMap<String, u64>,
    budget: &str,
) -> Result<(), Vec<String>> {
    let budget: Budget =
        serde_json::from_str(budget).map_err(|error| vec![format!("budget file: {error}")])?;
    let mut failures = Vec::new();
    if budget.version != BUDGET_VERSION {
        failures.push(format!(
            "budget version {} is not {BUDGET_VERSION}",
            budget.version
        ));
    }
    if budget.workload != workload {
        failures.push(format!(
            "budget workload {} is not {workload}",
            budget.workload
        ));
    }
    for (name, limit) in &budget.limits {
        match observed.get(name) {
            None => failures.push(format!("{name}: budgeted but not measured")),
            Some(value) if value > limit => {
                failures.push(format!("{name}: {value} exceeds the budget {limit}"));
            }
            Some(_) => {}
        }
    }
    for name in observed
        .keys()
        .filter(|name| !budget.limits.contains_key(*name))
    {
        failures.push(format!("{name}: measured but has no budget"));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

/// The observed counters in budget-file form, written next to every summary so a lowered
/// budget can be taken from a run.
pub(super) fn render(workload: &str, observed: &BTreeMap<String, u64>) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "version": BUDGET_VERSION,
        "workload": workload,
        "limits": observed,
    }))
    .expect("budget serializes")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counters(values: &[(&str, u64)]) -> BTreeMap<String, u64> {
        values.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
    }

    #[test]
    fn rendered_counters_pass_their_own_budget() {
        let observed = counters(&[("menu.draw_calls", 7), ("menu.bind_groups_created", 0)]);
        assert_eq!(check("w", &observed, &render("w", &observed)), Ok(()));
    }

    #[test]
    fn a_counter_above_its_limit_fails_and_below_passes() {
        let budget = render("w", &counters(&[("a", 10), ("b", 5)]));
        let failures = check("w", &counters(&[("a", 11), ("b", 4)]), &budget).unwrap_err();
        assert_eq!(failures, ["a: 11 exceeds the budget 10"]);
    }

    #[test]
    fn counter_sets_must_match_the_file() {
        let budget = render("w", &counters(&[("a", 1), ("gone", 1)]));
        let failures = check("w", &counters(&[("a", 1), ("new", 1)]), &budget).unwrap_err();
        assert_eq!(
            failures,
            [
                "gone: budgeted but not measured",
                "new: measured but has no budget"
            ]
        );
    }

    #[test]
    fn version_workload_and_syntax_are_checked() {
        let observed = counters(&[("a", 1)]);
        let stale = r#"{"version":0,"workload":"other","limits":{"a":1}}"#;
        let failures = check("w", &observed, stale).unwrap_err();
        assert_eq!(failures.len(), 2);
        assert!(check("w", &observed, "{").unwrap_err()[0].starts_with("budget file:"));
    }
}
