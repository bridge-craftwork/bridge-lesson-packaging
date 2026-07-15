//! `report` — human-readable summary over one or more collection profiles.
//!
//! Reads a profile JSON file or a directory of them and prints a comparative
//! table (percentages derived from the counts) plus a per-collection detail
//! block. Output is deterministic (collections sorted by name).

use crate::profile::CollectionProfile;
use anyhow::{bail, Context, Result};
use std::path::Path;

/// Load profiles from a file or directory and print the report.
pub fn report(path: &Path) -> Result<()> {
    let mut profiles = load(path)?;
    if profiles.is_empty() {
        bail!("no profiles found at {}", path.display());
    }
    profiles.sort_by(|a, b| a.collection.cmp(&b.collection));
    print_table(&profiles);
    for p in &profiles {
        print_detail(p);
    }
    Ok(())
}

fn load(path: &Path) -> Result<Vec<CollectionProfile>> {
    let mut paths = Vec::new();
    if path.is_dir() {
        for e in std::fs::read_dir(path).with_context(|| format!("reading {}", path.display()))? {
            let p = e?.path();
            if p.extension().map(|x| x == "json").unwrap_or(false) {
                paths.push(p);
            }
        }
    } else {
        paths.push(path.to_path_buf());
    }
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let txt = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
        out.push(
            serde_json::from_str(&txt).with_context(|| format!("parsing {}", p.display()))?,
        );
    }
    Ok(out)
}

/// Makeable = deals with a making designated contract (the difficulty ladder's
/// domain).
fn makeable(p: &CollectionProfile) -> usize {
    let d = &p.difficulty;
    d.cash_out_0 + d.establish_1 + d.technique_2 + d.unclassified
}

fn pct(n: usize, d: usize) -> String {
    if d == 0 {
        "  -".to_string()
    } else {
        format!("{:3.0}%", 100.0 * n as f64 / d as f64)
    }
}

fn print_table(profiles: &[CollectionProfile]) {
    println!(
        "{:<24} {:>6} {:>5} | {:>5} {:>5} {:>5} {:>5} | {:>5} {:>5} {:>5} | {:>5} {:>5}",
        "collection", "deals", "mkbl", "L0", "L1", "L2", "uncl", "auc", "cmt", "xcon", "fin", "ruff"
    );
    println!("{}", "-".repeat(100));
    for p in profiles {
        let mk = makeable(p);
        let d = &p.difficulty;
        println!(
            "{:<24} {:>6} {:>5} | {:>5} {:>5} {:>5} {:>5} | {:>5} {:>5} {:>5} | {:>5} {:>5}",
            truncate(&p.collection, 24),
            p.deal_count,
            pct(mk, p.deal_count),
            pct(d.cash_out_0, mk),
            pct(d.establish_1, mk),
            pct(d.technique_2, mk),
            pct(d.unclassified, mk),
            pct(p.structural.with_auction, p.deal_count),
            pct(p.structural.with_commentary, p.deal_count),
            pct(p.structural.with_explicit_contract, p.deal_count),
            p.techniques.get("finesse").copied().unwrap_or(0),
            p.techniques.get("ruff").copied().unwrap_or(0),
        );
    }
    println!(
        "\nL0 cash-out · L1 establish · L2 single-technique · uncl unclassified (% of makeable)\n\
         auc/cmt/xcon = % of deals with auction / commentary / explicit contract"
    );
}

fn print_detail(p: &CollectionProfile) {
    println!("\n=== {} ({} deals) ===", p.collection, p.deal_count);
    let d = &p.difficulty;
    if d.not_makeable + d.no_contract + d.incomplete > 0 {
        println!(
            "  excluded from ladder: not-makeable {}  no-contract {}  incomplete {}",
            d.not_makeable, d.no_contract, d.incomplete
        );
    }
    print_dist("  strain ", &p.contract_mix.by_strain);
    print_dist("  level  ", &p.contract_mix.by_level);
    print_dist("  declarer", &p.contract_mix.by_declarer);
    if !p.techniques.is_empty() {
        print_dist("  fired  ", &p.techniques);
    }
    if let Some(ed) = &p.editorial {
        println!("  editorial:");
        for (k, v) in [
            ("licensing", &ed.licensing),
            ("audience", &ed.intended_audience),
            ("commentary", &ed.commentary_quality),
            ("notes", &ed.notes),
        ] {
            if let Some(v) = v {
                println!("    {k}: {v}");
            }
        }
    }
}

fn print_dist(label: &str, map: &std::collections::BTreeMap<String, usize>) {
    if map.is_empty() {
        return;
    }
    let parts: Vec<String> = map.iter().map(|(k, v)| format!("{k}:{v}")).collect();
    println!("{label}: {}", parts.join("  "));
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n - 1])
    }
}
