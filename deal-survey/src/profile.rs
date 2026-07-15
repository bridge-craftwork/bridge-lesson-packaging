//! Stage 5 — roll-up. Folds the per-deal record ledger into one collection
//! profile: difficulty histogram, structural coverage, contract mix, technique
//! matrix, and an optional editorial block from a TOML sidecar.
//!
//! Counts only (no fractions) go in the JSON — exact and diffable; the `report`
//! command derives percentages for humans. All maps are `BTreeMap` so output is
//! deterministically ordered.

use crate::model::DealRecord;
use crate::topics::Topics;
use anyhow::{bail, Context, Result};
use bridge_types::Strain;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionProfile {
    pub collection: String,
    pub deal_count: usize,
    pub difficulty: DifficultyHistogram,
    pub structural: StructuralCoverage,
    pub auction: AuctionProfile,
    pub contract_mix: ContractMix,
    /// Probe name → how many deals it fired on.
    pub techniques: BTreeMap<String, usize>,
    /// Per-topic breakdown (baseline prior vs observed difficulty). Present only
    /// when a topic table was supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by_topic: Option<BTreeMap<String, TopicStats>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editorial: Option<Editorial>,
    pub versions: crate::model::Versions,
}

/// Difficulty per topic: the authored baseline prior alongside the observed,
/// objective difficulty — so the baselines can be calibrated against reality.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TopicStats {
    pub deal_count: usize,
    pub baseline_bidding: u8,
    pub baseline_cardplay: u8,
    /// Observed cardplay ladder among makeable deals: [L0, L1, L2].
    pub observed_cardplay: [usize; 3],
    pub observed_unclassified: usize,
    pub not_makeable: usize,
    /// Auction proxies observed within the topic.
    pub with_auction: usize,
    pub total_bids: usize,
    pub contested: usize,
    /// Combined cardplay difficulty = max(0, baseline + (observed_level - 1)),
    /// summed over makeable deals; `report` derives the mean. This is the DRAFT
    /// combination — baseline sets the floor, the observed ladder nudges ±1.
    pub combined_cardplay_sum: usize,
    pub combined_cardplay_n: usize,
}

/// Cardplay-difficulty histogram (mutually exclusive buckets over all deals).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DifficultyHistogram {
    pub cash_out_0: usize,
    pub establish_1: usize,
    pub technique_2: usize,
    pub unclassified: usize,
    /// Makeable-DD test failed (designated contract goes down double-dummy).
    pub not_makeable: usize,
    /// No contract could be resolved (no tag, no auction).
    pub no_contract: usize,
    /// Deal incomplete (not 52 cards) — no baseline.
    pub incomplete: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StructuralCoverage {
    pub with_auction: usize,
    pub with_play: usize,
    pub with_commentary: usize,
    pub with_explicit_contract: usize,
    /// Custom (collection-specific) tag → number of deals carrying it.
    pub custom_tags: BTreeMap<String, usize>,
}

/// Aggregate auction-complexity proxies (over deals that have an auction).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuctionProfile {
    pub with_auction: usize,
    /// Sum of contract-bid counts (for a mean).
    pub total_bids: usize,
    pub contested: usize,
    pub with_doubles: usize,
    pub total_doubles: usize,
    pub double_of_final: usize,
    pub with_high_bids: usize,
    pub total_high_bids: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContractMix {
    pub by_strain: BTreeMap<String, usize>,
    pub by_level: BTreeMap<String, usize>,
    pub by_declarer: BTreeMap<String, usize>,
}

/// Manually-supplied editorial metadata (TOML sidecar). All fields optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Editorial {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub licensing: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intended_audience: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commentary_quality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Build a profile from a directory of per-deal record JSON files.
pub fn build_from_dir(
    records_dir: &Path,
    editorial: Option<&Path>,
    topics: Option<&Path>,
) -> Result<CollectionProfile> {
    let mut records = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(records_dir)
        .with_context(|| format!("reading records dir {}", records_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    entries.sort();
    for path in entries {
        let txt = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let rec: DealRecord = serde_json::from_str(&txt)
            .with_context(|| format!("parsing {}", path.display()))?;
        records.push(rec);
    }
    if records.is_empty() {
        bail!("no records found in {}", records_dir.display());
    }

    let editorial = match editorial {
        Some(p) => Some(load_editorial(p)?),
        None => None,
    };
    let topics = match topics {
        Some(p) => Some(Topics::load(p)?),
        None => None,
    };
    Ok(build(records, editorial, topics.as_ref()))
}

/// Load an editorial TOML sidecar.
pub fn load_editorial(path: &Path) -> Result<Editorial> {
    let txt =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&txt).with_context(|| format!("parsing editorial TOML {}", path.display()))
}

/// Fold records into a profile. With a topic table, also produce the per-topic
/// baseline-vs-observed breakdown.
pub fn build(
    records: Vec<DealRecord>,
    editorial: Option<Editorial>,
    topics: Option<&Topics>,
) -> CollectionProfile {
    let collection = records
        .iter()
        .map(|r| r.source.collection.clone())
        .next()
        .unwrap_or_default();

    let mut difficulty = DifficultyHistogram::default();
    let mut structural = StructuralCoverage::default();
    let mut auction = AuctionProfile::default();
    let mut contract_mix = ContractMix::default();
    let mut techniques: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_topic: BTreeMap<String, TopicStats> = BTreeMap::new();

    for rec in &records {
        // Per-topic breakdown (baseline prior vs observed difficulty).
        if let Some(t) = topics {
            let (name, base) = t.resolve(&rec.source.file);
            let st = by_topic.entry(name).or_default();
            st.deal_count += 1;
            st.baseline_bidding = base.bidding;
            st.baseline_cardplay = base.cardplay;
            let a = &rec.structural.auction;
            if a.present {
                st.with_auction += 1;
                st.total_bids += a.bids as usize;
                if a.contested {
                    st.contested += 1;
                }
            }
            let makes = rec
                .baseline
                .as_ref()
                .and_then(|b| b.contract.as_ref())
                .map(|c| c.dd_makes)
                .unwrap_or(false);
            if !makes {
                st.not_makeable += 1;
            } else {
                match rec.cardplay.as_ref().and_then(|c| c.difficulty) {
                    Some(level @ 0..=2) => {
                        st.observed_cardplay[level as usize] += 1;
                        // Combined = baseline nudged ±1 by the observed ladder.
                        let combined = (base.cardplay as i32 + level as i32 - 1).max(0);
                        st.combined_cardplay_sum += combined as usize;
                        st.combined_cardplay_n += 1;
                    }
                    _ => st.observed_unclassified += 1,
                }
            }
        }

        // Structural coverage.
        let s = &rec.structural;
        let a = &s.auction;
        if a.present {
            structural.with_auction += 1;
            auction.with_auction += 1;
            auction.total_bids += a.bids as usize;
            auction.total_doubles += a.doubles as usize;
            auction.total_high_bids += a.high_bids as usize;
            if a.contested {
                auction.contested += 1;
            }
            if a.doubles > 0 {
                auction.with_doubles += 1;
            }
            if a.double_of_final {
                auction.double_of_final += 1;
            }
            if a.high_bids > 0 {
                auction.with_high_bids += 1;
            }
        }
        if s.play {
            structural.with_play += 1;
        }
        if s.commentary.present {
            structural.with_commentary += 1;
        }
        if matches!(s.contract_provenance, crate::model::ContractProvenance::Explicit) {
            structural.with_explicit_contract += 1;
        }
        for tag in &s.custom_tags {
            *structural.custom_tags.entry(tag.clone()).or_default() += 1;
        }

        // Difficulty histogram + contract mix.
        match rec.baseline.as_ref() {
            None => difficulty.incomplete += 1,
            Some(b) => match b.contract.as_ref() {
                None => difficulty.no_contract += 1,
                Some(facts) => {
                    if let Some((level, strain, declarer)) = parse_contract(&facts.contract) {
                        *contract_mix.by_strain.entry(strain).or_default() += 1;
                        *contract_mix.by_level.entry(level.to_string()).or_default() += 1;
                        *contract_mix.by_declarer.entry(declarer.to_string()).or_default() += 1;
                    }
                    if !facts.dd_makes {
                        difficulty.not_makeable += 1;
                    } else {
                        match rec.cardplay.as_ref().and_then(|c| c.difficulty) {
                            Some(0) => difficulty.cash_out_0 += 1,
                            Some(1) => difficulty.establish_1 += 1,
                            Some(2) => difficulty.technique_2 += 1,
                            _ => difficulty.unclassified += 1,
                        }
                    }
                }
            },
        }

        // Technique matrix.
        for probe in &rec.probes {
            if probe.fired {
                *techniques.entry(probe.name.clone()).or_default() += 1;
            }
        }
    }

    CollectionProfile {
        collection,
        deal_count: records.len(),
        difficulty,
        structural,
        auction,
        contract_mix,
        techniques,
        by_topic: topics.map(|_| by_topic),
        editorial,
        versions: crate::model::Versions::current(),
    }
}

/// Parse a contract display "4H by S" → (level, strain label, declarer char).
fn parse_contract(display: &str) -> Option<(u8, String, char)> {
    let (contract, decl) = display.split_once(" by ")?;
    let parsed = bridge_types::Contract::parse(contract)?;
    Some((parsed.level, strain_label(parsed.strain), decl.chars().next()?))
}

fn strain_label(s: Strain) -> String {
    match s {
        Strain::Clubs => "C",
        Strain::Diamonds => "D",
        Strain::Hearts => "H",
        Strain::Spades => "S",
        Strain::NoTrump => "NT",
    }
    .to_string()
}

/// Write a profile as `<out_dir>/<collection>.json`.
pub fn write(out_dir: &Path, profile: &CollectionProfile) -> Result<std::path::PathBuf> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("creating {}", out_dir.display()))?;
    let name = sanitize(&profile.collection);
    let path = out_dir.join(format!("{name}.json"));
    let json = serde_json::to_string_pretty(profile)?;
    std::fs::write(&path, format!("{json}\n"))
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn rec(difficulty: Option<u8>, dd_makes: bool, fired: &[&str], contract: &str) -> DealRecord {
        DealRecord {
            hash: "h".into(),
            source: Source { collection: "Test".into(), file: "f.pbn".into(), board: Some(1) },
            structural: Structural {
                contract: Some(contract.split_whitespace().next().unwrap().to_string()),
                contract_provenance: ContractProvenance::Explicit,
                auction: AuctionInfo {
                    present: true,
                    bids: 4,
                    contested: false,
                    doubles: 0,
                    double_of_final: false,
                    high_bids: 0,
                },
                play: false,
                commentary: Commentary { present: true, style: Some("inline".into()) },
                custom_tags: vec!["SkillPath".into()],
            },
            baseline: Some(Baseline {
                dd_table: DdTable { tricks: [[0; 5]; 4] },
                par: None,
                contract: Some(ContractFacts {
                    contract: contract.into(),
                    provenance: ContractProvenance::Explicit,
                    dd_tricks: if dd_makes { 10 } else { 8 },
                    required: 10,
                    dd_makes,
                    slack: 0,
                    declarer_seat_sensitive: false,
                }),
            }),
            cardplay: Some(Cardplay { immediate_winners: 5, required: 10, difficulty }),
            probes: fired
                .iter()
                .map(|n| ProbeRecord { name: n.to_string(), fired: true, evidence: serde_json::json!({}) })
                .collect(),
            versions: Versions::current(),
        }
    }

    #[test]
    fn aggregates_histogram_mix_and_techniques() {
        let p = build(
            vec![
                rec(Some(0), true, &[], "3NT by N"),
                rec(Some(2), true, &["finesse"], "4H by S"),
                rec(None, false, &[], "6S by S"), // not makeable
            ],
            None,
            None,
        );
        assert_eq!(p.deal_count, 3);
        assert_eq!(p.difficulty.cash_out_0, 1);
        assert_eq!(p.difficulty.technique_2, 1);
        assert_eq!(p.difficulty.not_makeable, 1);
        assert_eq!(p.techniques.get("finesse"), Some(&1));
        assert_eq!(p.structural.with_auction, 3);
        assert_eq!(p.contract_mix.by_strain.get("NT"), Some(&1));
        assert_eq!(p.contract_mix.by_strain.get("H"), Some(&1));
        assert_eq!(p.contract_mix.by_declarer.get("S"), Some(&2));
    }
}
