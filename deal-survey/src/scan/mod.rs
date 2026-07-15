//! Scan pipeline. Slice 1 wires stage 1 (structural); stages 2–4 land next.

pub mod structural;

use crate::hash::content_hash;
use crate::model::{DealRecord, Versions};
use anyhow::{Context, Result};
use bridge_types::{Deal, Direction};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Run the scan pipeline over a collection directory, writing one JSON record
/// per deal into `out_dir`. Returns the number of records written.
///
/// Slice 1: structural records only (no solver dependency).
pub fn scan_collection(collection_dir: &Path, out_dir: &Path) -> Result<usize> {
    let collection = collection_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| collection_dir.display().to_string());

    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("creating output dir {}", out_dir.display()))?;

    // Canonical form of an unparsed/empty deal, used to skip non-deal boards.
    let empty_deal = Deal::default().to_pbn(Direction::North);

    let mut written = 0;
    for pbn in pbn_files(collection_dir) {
        let rel = pbn
            .strip_prefix(collection_dir)
            .unwrap_or(&pbn)
            .to_string_lossy()
            .to_string();
        let boards = bridge_encodings::pbn::read_pbn_file(&pbn)
            .with_context(|| format!("parsing {}", pbn.display()))?;

        for board in &boards {
            // A board without a real deal can't be hashed or analyzed — skip it,
            // but don't fail the whole scan (read-only, best-effort catalog).
            let deal_pbn = board.deal.to_pbn(Direction::North);
            if deal_pbn == empty_deal {
                continue;
            }
            let record = DealRecord {
                hash: content_hash(&deal_pbn),
                source: structural::source_for(&collection, &rel, board.number),
                structural: structural::structural_of(board),
                baseline: None,
                cardplay: None,
                probes: Vec::new(),
                versions: Versions::current(),
            };
            write_record(out_dir, &record)?;
            written += 1;
        }
    }
    Ok(written)
}

fn pbn_files(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| p.extension().map(|x| x.eq_ignore_ascii_case("pbn")).unwrap_or(false))
        .collect();
    files.sort(); // deterministic output ordering
    files
}

/// One file per record, named by hash, pretty-printed for diffable ledgers.
fn write_record(out_dir: &Path, record: &DealRecord) -> Result<()> {
    let path = out_dir.join(format!("{}.json", record.hash));
    let json = serde_json::to_string_pretty(record)?;
    std::fs::write(&path, format!("{json}\n"))
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}
