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

/// Render one or more profiles as a self-contained HTML file.
pub fn write_html(path: &Path, out: &Path) -> Result<()> {
    let mut profiles = load(path)?;
    if profiles.is_empty() {
        bail!("no profiles found at {}", path.display());
    }
    profiles.sort_by(|a, b| a.collection.cmp(&b.collection));
    std::fs::write(out, crate::html::render(&profiles))
        .with_context(|| format!("writing {}", out.display()))?;
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
    // Auction-complexity proxies (means/percentages over deals with an auction).
    let a = &p.auction;
    if a.with_auction > 0 {
        let n = a.with_auction;
        println!(
            "  auction : {:.1} bids avg  ·  contested {}  ·  doubles {} ({} deals, {} final-dbl)  ·  high-bids {} ({} deals)",
            a.total_bids as f64 / n as f64,
            pct(a.contested, n),
            a.total_doubles,
            a.with_doubles,
            a.double_of_final,
            a.total_high_bids,
            a.with_high_bids,
        );
    }
    print_dist("  strain ", &p.contract_mix.by_strain);
    print_dist("  level  ", &p.contract_mix.by_level);
    print_dist("  declarer", &p.contract_mix.by_declarer);
    if !p.techniques.is_empty() {
        print_dist("  fired  ", &p.techniques);
    }
    if let Some(topics) = &p.by_topic {
        println!("  topics (baseline prior vs observed, + combined difficulty):");
        println!(
            "    {:<28} {:>5} {:>9} | {:>4} {:>4} {:>4} {:>5} | {:>7} {:>7}",
            "topic", "deals", "base b/cp", "L0", "L1", "L2", "n-mk", "comb-bid", "comb-cp"
        );
        let mean = |sum: usize, n: usize| {
            if n > 0 {
                format!("{:.1}", sum as f64 / n as f64)
            } else {
                "  -".to_string()
            }
        };
        for (name, t) in topics {
            println!(
                "    {:<28} {:>5} {:>4}/{:<4} | {:>4} {:>4} {:>4} {:>5} | {:>7} {:>7}",
                truncate(name, 28),
                t.deal_count,
                t.baseline_bidding,
                t.baseline_cardplay,
                t.observed_cardplay[0],
                t.observed_cardplay[1],
                t.observed_cardplay[2],
                t.not_makeable,
                mean(t.combined_bidding_sum, t.combined_bidding_n),
                mean(t.combined_cardplay_sum, t.combined_cardplay_n),
            );
        }
        println!("    (base b/cp = authored bidding/cardplay baseline; comb = baseline ± observed level, mean)");
    }
    print_lessons(p);
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

/// Per-lesson difficulty table, grouped by category in navigation order.
fn print_lessons(p: &CollectionProfile) {
    use crate::profile::TopicStats;
    if p.by_lesson.is_empty() {
        return;
    }
    let fmt = |sum: usize, n: usize| {
        if n > 0 {
            format!("{:.1}", sum as f64 / n as f64)
        } else {
            "  -".to_string()
        }
    };
    println!("  lessons by category (navigation order):");
    println!(
        "    {:<34} {:>5} | {:>4} {:>4} {:>4} {:>5} | {:>7} {:>7}  {}",
        "lesson", "deals", "L0", "L1", "L2", "n-mk", "comb-cp", "comb-bid", "topic"
    );

    // Categories alphabetical/hierarchical (BTreeMap = sorted keys); lessons in
    // full-path order within each.
    for (cat, roll) in p.by_category.iter() {
        println!(
            "  ▸ {}  ({} deals, cp {}, bid {})",
            cat,
            roll.deal_count,
            fmt(roll.combined_cardplay_sum, roll.combined_cardplay_n),
            fmt(roll.combined_bidding_sum, roll.combined_bidding_n),
        );
        let rows: Vec<(&String, &TopicStats)> =
            p.by_lesson.iter().filter(|(_, t)| &t.category == cat).collect();
        for (lesson, t) in rows {
            println!(
                "    {:<34} {:>5} | {:>4} {:>4} {:>4} {:>5} | {:>7} {:>7}  {}",
                truncate(&lesson_label(lesson), 34),
                t.deal_count,
                t.observed_cardplay[0],
                t.observed_cardplay[1],
                t.observed_cardplay[2],
                t.not_makeable,
                fmt(t.combined_cardplay_sum, t.combined_cardplay_n),
                fmt(t.combined_bidding_sum, t.combined_bidding_n),
                t.topic,
            );
        }
    }
}

/// Readable lesson label: basename without extension.
fn lesson_label(path: &str) -> String {
    let base = path.rsplit('/').next().unwrap_or(path);
    let base = base.strip_suffix(".pbn").unwrap_or(base);
    let base = base
        .strip_prefix("Baker Bridge ")
        .or_else(|| base.strip_prefix("thinking-bridge-"))
        .unwrap_or(base);
    base.strip_suffix(" practice deals").unwrap_or(base).to_string()
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
