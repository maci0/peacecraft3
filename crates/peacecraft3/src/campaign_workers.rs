//! Campaign script execution must also fit unoptimized development worker stacks.
pub(crate) fn initialize() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use bevy::{
            app::TaskPoolOptions,
            tasks::{ComputeTaskPool, TaskPoolBuilder},
        };
        let options = TaskPoolOptions::default();
        let total = bevy::tasks::available_parallelism()
            .clamp(options.min_total_threads, options.max_total_threads);
        let remaining = total.saturating_sub(assigned(&options.io, total, total));
        let remaining =
            remaining.saturating_sub(assigned(&options.async_compute, remaining, total));
        let count = assigned(&options.compute, remaining, total);
        ComputeTaskPool::get_or_init(|| {
            TaskPoolBuilder::default()
                .num_threads(count)
                // Local memory policy, not an authored game value; reservation is committed on demand.
                .stack_size(32 << 20)
                .thread_name("Compute Task Pool".into())
                .build()
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn assigned(
    policy: &bevy::app::TaskPoolThreadAssignmentPolicy,
    remaining: usize,
    total: usize,
) -> usize {
    ((total as f32 * policy.percent).round() as usize)
        .min(remaining)
        .clamp(policy.min_threads, policy.max_threads)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    #[test]
    fn worker_counts_keep_bevys_policy_including_small_machine_minima() {
        let policy = bevy::app::TaskPoolThreadAssignmentPolicy {
            min_threads: 2,
            max_threads: 6,
            percent: 0.3,
            on_thread_spawn: None,
            on_thread_destroy: None,
        };
        assert_eq!(assigned(&policy, 8, 20), 6);
        assert_eq!(assigned(&policy, 1, 20), 2);
        assert_eq!(assigned(&policy, 8, 11), 3);
    }
}
