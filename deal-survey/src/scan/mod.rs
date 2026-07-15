//! Scan pipeline: stage 1 (structural) + stage 2 (baseline DD) + stage 3
//! (cash-out). Probe stage (4) lands in slice 3.

pub mod structural;

mod cardplay;
mod contract;
mod dd;

use crate::hash::content_hash;
use crate::model::{
    Baseline, Cardplay, ContractFacts, DealRecord, Versions, LADDER_VERSION, TOOL_VERSION,
};
use anyhow::{Context, Result};
use bridge_types::{Board, Deal, Direction};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Outcome counts for one collection scan.
#[derive(Debug, Default)]
pub struct ScanSummary {
    pub total: usize,
    pub written: usize,
    pub cached: usize,
    /// Records that got a DD baseline (complete 52-card deal).
    pub baselined: usize,
    /// Records with an analyzed contract (explicit or inferred).
    pub with_contract: usize,
    /// Cardplay difficulty 0 (cash-out) among analyzed contracts.
    pub difficulty0: usize,
}

/// Run the scan pipeline over a collection directory, writing one JSON record
/// per deal into `out_dir`. Skips deals already fully baselined at the current
/// tool/ladder version.
pub fn scan_collection(collection_dir: &Path, out_dir: &Path) -> Result<ScanSummary> {
    let collection = collection_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| collection_dir.display().to_string());

    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("creating output dir {}", out_dir.display()))?;

    // Canonical form of an unparsed/empty deal, used to skip non-deal boards.
    let empty_deal = Deal::default().to_pbn(Direction::North);

    let mut sum = ScanSummary::default();
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
            sum.total += 1;
            let hash = content_hash(&deal_pbn);

            // Cache: skip deals already fully baselined at this tool/ladder.
            if let Some(existing) = cached_record(out_dir, &hash) {
                if is_current_baselined(&existing) {
                    sum.cached += 1;
                    tally(&mut sum, &existing);
                    continue;
                }
            }

            let (baseline, cardplay) = analyze(board);
            let record = DealRecord {
                hash,
                source: structural::source_for(&collection, &rel, board.number),
                structural: structural::structural_of(board),
                baseline,
                cardplay,
                probes: Vec::new(),
                versions: Versions::current(),
            };
            write_record(out_dir, &record)?;
            sum.written += 1;
            tally(&mut sum, &record);
        }
    }
    Ok(sum)
}

/// Stage 2 + 3 for one board. Baseline is present for any complete deal;
/// contract facts and cardplay only when a contract is resolvable.
fn analyze(board: &Board) -> (Option<Baseline>, Option<Cardplay>) {
    if !dd::is_complete(&board.deal) {
        return (None, None);
    }
    let dd_table = dd::solve_dd_table(&board.deal);

    let (contract_facts, cardplay) = match contract::effective_contract(board) {
        Some(ec) => {
            let dd_tricks = dd_table.get(ec.declarer, ec.strain);
            let required = ec.level + 6;
            let dd_makes = dd_tricks >= required;
            let partner = partner_of(ec.declarer);
            let facts = ContractFacts {
                contract: ec.display,
                provenance: ec.provenance,
                dd_tricks,
                required,
                dd_makes,
                slack: dd_tricks as i32 - required as i32,
                declarer_seat_sensitive: dd_table.get(partner, ec.strain) != dd_tricks,
            };
            // Cash-out: declarer + dummy immediate top winners.
            let winners = cardplay::immediate_top_winners(
                board.deal.hand(ec.declarer),
                board.deal.hand(partner),
            );
            // Slice 2 classifies only level 0; the rest awaits slice-3 probes.
            let difficulty = if dd_makes && winners >= required {
                Some(0)
            } else {
                None
            };
            let cp = Cardplay {
                immediate_winners: winners,
                required,
                difficulty,
            };
            (Some(facts), Some(cp))
        }
        None => (None, None),
    };

    let baseline = Baseline {
        dd_table,
        par: None, // competitive par deferred (bridge-solver has none yet)
        contract: contract_facts,
    };
    (Some(baseline), cardplay)
}

fn partner_of(d: Direction) -> Direction {
    match d {
        Direction::North => Direction::South,
        Direction::South => Direction::North,
        Direction::East => Direction::West,
        Direction::West => Direction::East,
    }
}

fn tally(sum: &mut ScanSummary, rec: &DealRecord) {
    if rec.baseline.is_some() {
        sum.baselined += 1;
    }
    if rec.baseline.as_ref().and_then(|b| b.contract.as_ref()).is_some() {
        sum.with_contract += 1;
    }
    if rec.cardplay.as_ref().and_then(|c| c.difficulty) == Some(0) {
        sum.difficulty0 += 1;
    }
}

fn cached_record(out_dir: &Path, hash: &str) -> Option<DealRecord> {
    let path = out_dir.join(format!("{hash}.json"));
    let txt = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<DealRecord>(&txt).ok()
}

fn is_current_baselined(rec: &DealRecord) -> bool {
    rec.versions.tool == TOOL_VERSION
        && rec.versions.ladder == LADDER_VERSION
        && rec.baseline.is_some()
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
