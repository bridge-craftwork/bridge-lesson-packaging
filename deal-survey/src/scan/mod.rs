//! Scan pipeline: stage 1 (structural) + stage 2 (baseline DD) + stage 3
//! (cash-out). Probe stage (4) lands in slice 3.

pub mod structural;

mod cardplay;
mod contract;
mod dd;
mod probe;

use crate::hash::content_hash;
use crate::model::{
    Baseline, Cardplay, ContractFacts, DealRecord, Par, ProbeRecord, Versions, LADDER_VERSION,
    TOOL_VERSION,
};
use anyhow::{Context, Result};
use bridge_types::{Board, Deal, Direction, Vulnerability};
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
    /// Cardplay difficulty histogram among makeable contracts: [d0, d1, d2].
    pub difficulty: [usize; 3],
    /// Makeable contracts left unclassified (ambiguous / multiple probes fire).
    pub unclassified: usize,
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
            let hash = content_hash(&board.deal); // rotation-canonical

            // Cache: skip deals already fully baselined at this tool/ladder.
            if let Some(existing) = cached_record(out_dir, &hash) {
                if is_current_baselined(&existing) {
                    sum.cached += 1;
                    tally(&mut sum, &existing);
                    continue;
                }
            }

            let (baseline, cardplay, probes) = analyze(board);
            let record = DealRecord {
                hash,
                source: structural::source_for(&collection, &rel, board),
                structural: structural::structural_of(board),
                baseline,
                cardplay,
                probes,
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
/// contract facts, cardplay, and probes only when a contract is resolvable.
fn analyze(board: &Board) -> (Option<Baseline>, Option<Cardplay>, Vec<ProbeRecord>) {
    if !dd::is_complete(&board.deal) {
        return (None, None, Vec::new());
    }
    let (dd_table, bs_table) = dd::solve(&board.deal);

    // Competitive par (reuses the same solved table).
    let (vul_ns, vul_ew) = match board.vulnerable {
        Vulnerability::None => (false, false),
        Vulnerability::NorthSouth => (true, false),
        Vulnerability::EastWest => (false, true),
        Vulnerability::Both => (true, true),
    };
    let par_res = bridge_solver::par(&bs_table, vul_ns, vul_ew);
    let par = Some(Par {
        optimum_score: par_res.optimum_score(),
        contract: par_res.contract.map(|c| c.describe()),
    });

    let (contract_facts, cardplay, probes) = match contract::effective_contract(board) {
        Some(ec) => {
            let dd_tricks = dd_table.get(ec.declarer, ec.strain);
            let required = ec.level + 6;
            let dd_makes = dd_tricks >= required;
            let partner = partner_of(ec.declarer);
            let facts = ContractFacts {
                contract: ec.display.clone(),
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
            // Ladder: 0 = cash-out; else probe. Probes only run on makeable,
            // non-cash-out contracts (per spec, gated on the baseline).
            let cashout = dd_makes && winners >= required;
            let (difficulty, probes) = if !dd_makes {
                (None, Vec::new()) // down DD — making difficulty is moot
            } else if cashout {
                (Some(0), Vec::new())
            } else {
                let recs = probe::run_probes(&board.deal, &ec, required, dd_tricks);
                let fired = recs.iter().filter(|r| r.fired).count();
                // 0 fired → establish/drive-out (1); exactly 1 → single technique
                // (2); 2+ → unclassified (ambiguous), never a wrong level.
                let diff = match fired {
                    0 => Some(1),
                    1 => Some(2),
                    _ => None,
                };
                (diff, recs)
            };
            let cp = Cardplay {
                immediate_winners: winners,
                required,
                difficulty,
            };
            (Some(facts), Some(cp), probes)
        }
        None => (None, None, Vec::new()),
    };

    let baseline = Baseline {
        dd_table,
        par,
        contract: contract_facts,
    };
    (Some(baseline), cardplay, probes)
}

fn partner_of(d: Direction) -> Direction {
    match d {
        Direction::North => Direction::South,
        Direction::South => Direction::North,
        Direction::East => Direction::West,
        Direction::West => Direction::East,
    }
}

#[cfg(test)]
mod calibration {
    use super::analyze;

    /// The checked-in calibration set: hand-labeled deals spanning levels 0–2
    /// (ground truth = Baker lesson intent + DD-verified probe evidence).
    const PBN: &str = include_str!("../../calibration/calibration.pbn");

    /// Acceptance (spec): probe verdicts match the labels on ≥90% of the set,
    /// with NO silent misclassification — a disagreement must land in
    /// `unclassified` (None), never at a different definite level.
    #[test]
    fn ladder_matches_calibration_labels() {
        let boards = bridge_encodings::pbn::read_pbn(PBN).expect("calibration parses");
        assert!(boards.len() >= 12, "calibration set too small");

        let mut matched = 0;
        let mut wrong = Vec::new();
        for board in &boards {
            let expected: u8 = board
                .extra_tag("ExpectedDifficulty")
                .and_then(|v| v.parse().ok())
                .expect("every calibration deal labels ExpectedDifficulty");
            let technique = board.extra_tag("ExpectedTechnique").unwrap_or("");
            let got = analyze(board).1.and_then(|c| c.difficulty);

            match got {
                Some(d) if d == expected => matched += 1,
                Some(d) => wrong.push(format!(
                    "board {:?} ({technique}): expected {expected}, got {d}",
                    board.board_id
                )),
                None => {} // unclassified — allowed disagreement, not a wrong level
            }
        }

        assert!(
            wrong.is_empty(),
            "silent misclassification(s) — forbidden:\n{}",
            wrong.join("\n")
        );
        let frac = matched as f64 / boards.len() as f64;
        assert!(
            frac >= 0.90,
            "calibration match {matched}/{} = {:.0}% < 90%",
            boards.len(),
            frac * 100.0
        );
    }
}

fn tally(sum: &mut ScanSummary, rec: &DealRecord) {
    if rec.baseline.is_some() {
        sum.baselined += 1;
    }
    let facts = rec.baseline.as_ref().and_then(|b| b.contract.as_ref());
    if facts.is_some() {
        sum.with_contract += 1;
    }
    // Difficulty histogram covers makeable contracts only.
    if facts.map(|f| f.dd_makes).unwrap_or(false) {
        match rec.cardplay.as_ref().and_then(|c| c.difficulty) {
            Some(d @ 0..=2) => sum.difficulty[d as usize] += 1,
            _ => sum.unclassified += 1,
        }
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
