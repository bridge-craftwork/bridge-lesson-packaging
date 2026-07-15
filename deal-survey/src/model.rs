//! Per-deal characterization record — the wire schema (see deal-survey-spec.md).
//!
//! These serde structs ARE the contract: the emitted JSON must round-trip. Stages
//! fill in their block; blocks not yet computed serialize as absent (never as a
//! silent guess). `versions` drives cache invalidation when the ladder/probe logic
//! changes.

use serde::{Deserialize, Serialize};

/// One JSON object per deal, keyed by content hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DealRecord {
    pub hash: String,
    pub source: Source,
    pub structural: Structural,

    // Filled by later stages (slices 2–3). Absent until computed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<Baseline>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardplay: Option<Cardplay>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probes: Vec<ProbeRecord>,

    pub versions: Versions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub collection: String,
    pub file: String,
    pub board: Option<u32>,
}

/// Stage 1 — what the PBN actually contains. No solves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Structural {
    /// e.g. "4H by S" — present only when the PBN designates one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
    pub contract_provenance: ContractProvenance,
    pub auction: bool,
    pub play: bool,
    pub commentary: Commentary,
    /// Non-standard tag names present on the deal (e.g. bridge-mastery tags).
    pub custom_tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContractProvenance {
    /// A `[Contract]` tag was present.
    Explicit,
    /// No `[Contract]`, but derivable from the auction (slice 2 policy). Reserved.
    Inferred,
    /// No `[Contract]` and none derivable — DD contract analysis is skipped, not guessed.
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commentary {
    pub present: bool,
    /// "inline" (`{...}`), "line" (`%`/`;`), or absent when no commentary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
}

/// Stage 2 — baseline DD (1 full solve/deal). Reserved for slice 2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    /// 20-entry DD table, encoded as declarer×denomination trick counts.
    pub dd_table: serde_json::Value,
    pub par: String,
    pub contract_dd_makes: bool,
    pub slack: i32,
}

/// Stage 3 — cash-out check + ladder assignment. Reserved for slices 2–3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cardplay {
    pub immediate_winners: u8,
    pub required: u8,
    /// 0/1/2 on the v1 ladder; `None` == unclassified (an honest output, not an error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<u8>,
}

/// Stage 4 — one entry per probe run. Reserved for slice 3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeRecord {
    pub name: String,
    pub fired: bool,
    pub evidence: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Versions {
    pub tool: String,
    pub ladder: u32,
}

/// Tool version stamped into every record.
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Difficulty-ladder version — bump when ladder/probe logic changes (cache key).
pub const LADDER_VERSION: u32 = 1;

impl Versions {
    pub fn current() -> Self {
        Versions {
            tool: TOOL_VERSION.to_string(),
            ladder: LADDER_VERSION,
        }
    }
}
