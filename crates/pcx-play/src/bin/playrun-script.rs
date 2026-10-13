//! Fail-closed execution probe for local extracted common.j/Blizzard.j/war3map.j.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    use pcx_play::scripts::{AssetIdentity, MapScripts, ScriptAsset};
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(args.len() == 6 || args.len() == 7) {
        eprintln!(
            "usage: playrun-script <edition> <version> <asset-source> <common.j> <Blizzard.j> <war3map.j> [war3map.wts]"
        );
        return std::process::ExitCode::from(2);
    }
    let result = (|| -> Result<(), String> {
        let mut bytes = Vec::new();
        for path in &args[3..] {
            bytes.push(std::fs::read(path).map_err(|e| format!("{path}: {e}"))?);
        }
        let asset = |i: usize| ScriptAsset {
            label: args[i + 3].as_str(),
            bytes: bytes[i].as_slice(),
        };
        let scripts = MapScripts::compile(
            AssetIdentity {
                edition: args[0].clone(),
                version: args[1].clone(),
                source: args[2].clone(),
            },
            asset(0),
            asset(1),
            asset(2),
            (bytes.len() == 4).then(|| asset(3)),
        )?;
        println!(
            "resolved edition={:?} version={:?} source={:?} functions={} natives={}",
            scripts.identity.edition,
            scripts.identity.version,
            scripts.identity.source,
            scripts.program().functions().len(),
            scripts.program().natives().len()
        );
        for digest in &scripts.digests {
            println!("input {:?} fnv1a64={:016x}", digest.label, digest.hash);
        }
        scripts.probe()?;
        println!("config/main executed; campaign outcomes were not evaluated");
        Ok(())
    })();
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("playrun-script: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
#[cfg(target_arch = "wasm32")]
fn main() {}
