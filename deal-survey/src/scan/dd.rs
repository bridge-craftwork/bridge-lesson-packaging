//! Stage 2 — baseline double-dummy solve. One 20-entry table per deal.
//!
//! Delegates to `bridge_solver::solve_dd_table` (the shared driver) and adapts
//! its `DdTricks` into this crate's `DdTable` — identical layout (seat N,E,S,W ×
//! strain C,D,H,S,NT), so it's a field move.

use crate::model::DdTable;
use bridge_types::Deal;

/// True only for a complete 52-card deal (DD solving requires all four hands).
pub fn is_complete(deal: &Deal) -> bool {
    [&deal.north, &deal.east, &deal.south, &deal.west]
        .iter()
        .all(|h| h.len() == 13)
}

/// Solve the full 20-entry DD table for a complete deal, plus the raw
/// `bridge_solver` table (reused directly for par without re-solving).
pub fn solve(deal: &Deal) -> (DdTable, bridge_solver::DdTricks) {
    let bs = bridge_solver::solve_dd_table(deal);
    (DdTable { tricks: bs.tricks }, bs)
}
