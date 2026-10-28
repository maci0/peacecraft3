//! Mounting archives as asset layers and resolving members, as the browser and tools do.
use std::collections::BTreeSet;

use pcx_assets::{ArchiveLayer, AssetPath, GameMode, MpqSource, NamedMember, Resolver};
use pcx_mpq::{ExtractionBudget, MpqArchive};

use crate::{Context, Failure, Outcome, Prepared, Workload, digest::Fnv, input};

pub const WORKLOADS: [Workload; 1] = [Workload {
    name: "resolver-mount-v1",
    summary: "MpqSource and Resolver over listed unit, script and UI members, then 128 resolves",
    prepare: mount,
}];

/// Listed members under these directories are mounted.
const MOUNTED_PREFIXES: [&str; 3] = ["units\\", "scripts\\", "ui\\framedef\\"];

/// Mounted members read back through the resolver, per run.
const RESOLVED_MEMBERS: usize = 128;

const BUDGET: ExtractionBudget = ExtractionBudget {
    max_raw_packed_bytes: 64 << 20,
    max_decoded_bytes: 64 << 20,
};

struct Mount {
    archive: Option<MpqArchive>,
    id: String,
    names: Vec<String>,
}

fn mount(ctx: &Context) -> Result<Prepared, Failure> {
    let table = input::codec_table(&ctx.install)?;
    let mut input_digest = Fnv::default();
    let mut mounts = Vec::new();
    for path in input::archive_paths(&ctx.install)? {
        let mut archive = input::open_archive(&path)?;
        let listing = archive
            .extract_bounded("(listfile)", BUDGET, Some(&table))
            .map_err(|e| e.to_string())?;
        let listing = String::from_utf8(listing).map_err(|e| e.to_string())?;
        let mut seen = BTreeSet::new();
        let mut names = Vec::new();
        for line in listing.lines().map(str::trim) {
            let low = line.to_ascii_lowercase();
            if MOUNTED_PREFIXES.iter().any(|p| low.starts_with(p))
                && archive.contains(line)
                && AssetPath::new(line).is_ok_and(|p| seen.insert(p))
            {
                names.push(line.to_string());
            }
        }
        let id = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        input_digest.text(&id);
        names.iter().for_each(|n| input_digest.text(n));
        mounts.push(Mount {
            archive: Some(archive),
            id,
            names,
        });
    }
    let resolved: Vec<String> = mounts
        .iter()
        .flat_map(|m| m.names.iter().cloned())
        .step_by(7)
        .take(RESOLVED_MEMBERS)
        .collect();
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let mut layers = Vec::new();
            let mut mounted = 0u64;
            for (priority, mount) in (0u32..).zip(mounts.iter_mut()) {
                let archive = mount.archive.take().ok_or("workload ran twice")?;
                let names: Vec<NamedMember> = mount
                    .names
                    .iter()
                    .map(|n| NamedMember {
                        asset: n.clone(),
                        member: n.clone(),
                    })
                    .collect();
                mounted += names.len() as u64;
                let source = MpqSource::new_bounded(
                    archive,
                    mount.id.clone(),
                    Some("fixed workload".into()),
                    names,
                    BUDGET,
                    Some((*table).clone()),
                )
                .map_err(|e| e.to_string())?;
                // Later archives in the list have lower priority.
                layers.push(ArchiveLayer {
                    priority: 1000 - priority,
                    source: Box::new(source),
                });
            }
            let resolver =
                Resolver::new(GameMode::default(), None, layers).map_err(|e| e.to_string())?;
            let mut assets = Vec::new();
            for name in &resolved {
                assets.push(
                    resolver
                        .resolve_bounded(name, BUDGET.max_decoded_bytes as u64)
                        .map_err(|e| e.to_string())?,
                );
            }
            let paths = resolver.paths().count() as u64;
            Ok(Outcome {
                work: vec![
                    ("mounted", mounted),
                    ("paths", paths),
                    ("resolved", assets.len() as u64),
                ],
                fold: Box::new(move |out| {
                    out.u64(paths);
                    for asset in &assets {
                        out.text(asset.path.as_str());
                        out.bytes(&asset.bytes);
                        out.debug(&asset.provenance);
                    }
                }),
            })
        }),
    })
}
