//! Emit a bounded synthetic movement-v1 RPL to stdout; no original assets read.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    use std::io::Write;
    match execute().and_then(|source| {
        std::io::stdout()
            .lock()
            .write_all(source.as_bytes())
            .map_err(|e| e.to_string())
    }) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("playgen: {error}");
            std::process::ExitCode::from(2)
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn execute() -> Result<String, String> {
    use pcx_play::{
        AssetMode, RunConfig,
        generator::{ActionKind, Limits, MovementSpec, generate},
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 11 {
        return Err("usage: playgen SEED TICKS UNITS ACTIONS MIN_X MAX_X MIN_Y MAX_Y MAX_WORK MAX_BYTES OWNER [OWNER ...]".into());
    }
    fn number<T: std::str::FromStr>(
        args: &[String],
        index: usize,
        name: &str,
    ) -> Result<T, String> {
        args[index]
            .parse()
            .map_err(|_| format!("invalid {name}: {:?}", args[index]))
    }
    let seed = number(&args, 0, "seed")?;
    let ticks = number(&args, 1, "ticks")?;
    let units = number(&args, 2, "units")?;
    let actions = number(&args, 3, "actions")?;
    let min_x = number(&args, 4, "min x")?;
    let max_x = number(&args, 5, "max x")?;
    let min_y = number(&args, 6, "min y")?;
    let max_y = number(&args, 7, "max y")?;
    let max_work = number(&args, 8, "max work")?;
    let max_output_bytes = number(&args, 9, "max bytes")?;
    let owners = args[10..]
        .iter()
        .map(|value| {
            value
                .parse::<u8>()
                .map_err(|_| format!("invalid owner: {value:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    generate(
        &MovementSpec {
            scenario: "generated-movement-v1",
            map: "synthetic.generated-movement-v1",
            ticks,
            units,
            actions,
            owners: &owners,
            x: min_x..=max_x,
            y: min_y..=max_y,
            action_kinds: &[
                ActionKind::Move,
                ActionKind::Select,
                ActionKind::Stop,
                ActionKind::Nop,
                ActionKind::RngDraw,
            ],
        },
        &RunConfig {
            asset_mode: AssetMode::Vanilla,
            seed,
            fix_rows: Vec::new(),
        },
        Limits {
            // CLI resource policy, not native unit/action limits.
            max_units: 1024,
            max_actions: 16_384,
            max_ticks: max_work,
            max_fix_rows: 0,
            max_work,
            max_output_bytes,
        },
    )
    .map_err(|e| e.to_string())
}
