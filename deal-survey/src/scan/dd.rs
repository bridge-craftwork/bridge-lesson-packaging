//! Stage 2 — baseline double-dummy solve. One 20-entry table per deal.
//!
//! Delegates to `bridge_solver::solve_dd_table` (the shared driver) and adapts
//! its `DdTricks` into this crate's `DdTable` — identical layout (seat N,E,S,W ×
//! strain C,D,H,S,NT), so it's a field move.

use crate::model::DdTable;
use bridge_solver::{direction_to_seat, Hands, Solver, CLUB, DIAMOND, HEART, NOTRUMP, SPADE};
use bridge_types::{Deal, Direction, Strain};

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

/// Solve a single contract: DD tricks the `declarer` takes in `strain`. Used to
/// evaluate perturbed deals in the probe pass (one solve each).
pub fn solve_contract(deal: &Deal, strain: Strain, declarer: Direction) -> u8 {
    let hands = Hands::from_deal(deal);
    let total = hands.num_tricks() as u8;
    let seat = direction_to_seat(declarer);
    let leader = (seat + 1) % 4;
    let ns = Solver::new(hands, strain_trump(strain), leader).solve();
    if matches!(declarer, Direction::North | Direction::South) {
        ns
    } else {
        total - ns
    }
}

fn strain_trump(strain: Strain) -> usize {
    match strain {
        Strain::Clubs => CLUB,
        Strain::Diamonds => DIAMOND,
        Strain::Hearts => HEART,
        Strain::Spades => SPADE,
        Strain::NoTrump => NOTRUMP,
    }
}
