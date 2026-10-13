//! `slqx` — inspect a `.slk` table's records, grid and decoded digest.
//!
//! The parser refuses anything it cannot prove, so this tool is how a variant
//! gets looked at before deciding what it is.
//!
//! ```text
//! slqx info <file.slk>
//! slqx dump <file.slk> [rows] [cols]
//! slqx hash <file.slk> [raw]
//! ```

use std::process::ExitCode;

use pcx_slq::{Severity, Table};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: slqx info|dump|hash <file.slk> [args]";
    let Some(cmd) = args.first().map(String::as_str) else {
        eprintln!("{usage}");
        return ExitCode::from(2);
    };
    if args.len() < 2 {
        eprintln!("{usage}");
        return ExitCode::from(2);
    }
    let path = &args[1];
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{path}: {e}");
            return ExitCode::from(1);
        }
    };

    if cmd == "hash" {
        let raw = args.get(2).map(String::as_str) == Some("raw");
        let t = match Table::parse(&data) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("{path}: {e}");
                return ExitCode::from(1);
            }
        };
        if raw {
            println!(
                "{} file bytes fnv1a64 {:016x}",
                data.len(),
                pcx_slq::fnv1a64(&data)
            );
            return ExitCode::SUCCESS;
        }
        let canon = t.canonical();
        println!(
            "{} cells={} canonical={} fnv1a64 {:016x}",
            path,
            t.cell_count(),
            canon.len(),
            pcx_slq::fnv1a64(&canon)
        );
        println!("  format: Y<row>;X<col>;K<value>\\n, rows then columns ascending");
        return ExitCode::SUCCESS;
    }

    let t = match Table::parse(&data) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{path}: {e}");
            return ExitCode::from(1);
        }
    };

    if cmd == "info" {
        println!("{path}");
        println!("  bytes            {}", data.len());
        println!(
            "  signature        {}",
            String::from_utf8_lossy(pcx_slq::SIGNATURE)
        );
        match t.declared() {
            Some(d) => println!(
                "  declared (B)     {} columns x {} rows, date {}{}",
                d.columns,
                d.rows,
                d.date,
                if d.date_extras.is_empty() {
                    String::new()
                } else {
                    format!(" {:?}", d.date_extras)
                }
            ),
            None => println!("  declared (B)     absent"),
        }
        println!(
            "  actual           {} columns x {} rows ({} cells)",
            t.column_count(),
            t.row_count(),
            t.cell_count()
        );
        let counts = t.record_counts();
        let mut line = String::from("  records         ");
        for (letter, n) in counts {
            let _ = std::fmt::Write::write_fmt(&mut line, format_args!(" {letter}={n}"));
        }
        println!("{line}");
        println!("  headers");
        for (column, name) in t.headers() {
            println!("    X{column:<4} {name:?}");
        }
        let errors = t
            .anomalies()
            .iter()
            .filter(|a| a.severity != Severity::Info)
            .count();
        let info = t.anomalies().len() - errors;
        println!("  generated cells  {}", t.generated_cells().len());
        println!("  anomalies       {errors} notable, {info} informational");
        for a in t
            .anomalies()
            .iter()
            .filter(|a| a.severity != Severity::Info)
        {
            println!(
                "    {:?}/{:?} row {:?} col {:?}: {}",
                a.kind, a.severity, a.row, a.column, a.detail
            );
        }
        println!("  fnv1a64 {:016x}", t.fnv1a64());
        return ExitCode::SUCCESS;
    }

    if cmd == "dump" {
        let want_rows: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(8);
        let want_cols: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(u32::MAX);
        let cols = want_cols.min(t.column_count());
        let last = want_rows.min(t.row_count());
        println!(
            "{path}: rows 1..={last} of {}, columns 1..={cols} of {}",
            t.row_count(),
            t.column_count()
        );
        for row in t.rows() {
            if row.index() > last {
                break;
            }
            let label = if row.index() == 1 { "header" } else { "row   " };
            print!("  {label} {:>5}", row.index());
            for column in 1..=cols {
                match row.get(column) {
                    Some(v) => print!(" | X{column}={v:?}"),
                    None => print!(" | X{column}=<none>"),
                }
            }
            println!();
        }
        return ExitCode::SUCCESS;
    }

    eprintln!("unknown command {cmd}");
    eprintln!("{usage}");
    ExitCode::from(2)
}
