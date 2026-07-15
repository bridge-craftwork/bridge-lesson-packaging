//! Per-deal characterization record — the wire schema (see deal-survey-spec.md).
//!
//! These serde structs ARE the contract: the emitted JSON must round-trip. Stages
//! fill in their block; blocks not yet computed serialize as absent (never as a
//! silent guess). `versions` drives cache invalidation when the ladder/probe logic
//! changes.

use bridge_types::{Direction, Strain};
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
    /// 20-entry DD table: max tricks per (declarer seat) × (strain).
    pub dd_table: DdTable,
    /// Competitive par (score + contracts). DEFERRED — bridge-solver does not
    /// implement par yet (requires game-theoretic competitive search); `None`
    /// until it lands, never a guess.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub par: Option<String>,
    /// Facts about the designated contract, when one is present/inferable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract: Option<ContractFacts>,
}

/// DD-derived facts for the designated contract in the baseline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractFacts {
    /// The effective contract analyzed, e.g. "4H by S".
    pub contract: String,
    /// How the contract was obtained (explicit tag vs auction-inferred).
    pub provenance: ContractProvenance,
    /// DD tricks declarer takes in the contract strain.
    pub dd_tricks: u8,
    /// Tricks required to make (level + 6).
    pub required: u8,
    pub dd_makes: bool,
    /// DD tricks minus required (negative = DD goes down).
    pub slack: i32,
    /// Whether the DD result changes if the partner declares the same strain
    /// (a rough lead-sensitivity signal).
    pub declarer_seat_sensitive: bool,
}

/// 20-entry double-dummy table. `tricks[declarer][strain]` in the fixed order
/// declarer = N,E,S,W and strain = C,D,H,S,NT (bridge-types enum order).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DdTable {
    pub tricks: [[u8; 5]; 4],
}

impl DdTable {
    pub fn get(&self, declarer: Direction, strain: Strain) -> u8 {
        self.tricks[dir_index(declarer)][strain_index(strain)]
    }
}

/// Fixed declarer index: N,E,S,W (bridge-types `Direction` order).
pub fn dir_index(d: Direction) -> usize {
    match d {
        Direction::North => 0,
        Direction::East => 1,
        Direction::South => 2,
        Direction::West => 3,
    }
}

/// Fixed strain index: C,D,H,S,NT (bridge-types `Strain` order).
pub fn strain_index(s: Strain) -> usize {
    match s {
        Strain::Clubs => 0,
        Strain::Diamonds => 1,
        Strain::Hearts => 2,
        Strain::Spades => 3,
        Strain::NoTrump => 4,
    }
}

/// Stage 3 — cash-out check + ladder assignment. Present only when a contract
/// is analyzed. Reserved for extension by slice-3 probes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cardplay {
    /// Immediate top winners for declarer's side (cash-out heuristic).
    pub immediate_winners: u8,
    pub required: u8,
    /// v1 ladder level. Slice 2 assigns only `0` (cash-out) or `None`
    /// (unclassified — pending slice-3 probes); never a wrong level.
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
