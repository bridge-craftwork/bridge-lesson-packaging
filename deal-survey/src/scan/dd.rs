//! Stage 2 — baseline double-dummy solve. One 20-entry table per deal.
//!
//! Replicates bridge-solver's own driver: caches are per-trump and shared
//! across the four declarer seats (matching the C++ engine's contract).

use crate::model::{dir_index, strain_index, DdTable};
use bridge_solver::{
    direction_to_seat, CutoffCache, Hands, PatternCache, Solver, CLUB, DIAMOND, HEART, NOTRUMP,
    SPADE,
};
use bridge_types::{Deal, Direction, Strain};

const DIRECTIONS: [Direction; 4] = [
    Direction::North,
    Direction::East,
    Direction::South,
    Direction::West,
];
const STRAINS: [Strain; 5] = [
    Strain::Clubs,
    Strain::Diamonds,
    Strain::Hearts,
    Strain::Spades,
    Strain::NoTrump,
];

/// True only for a complete 52-card deal (DD solving requires all four hands).
pub fn is_complete(deal: &Deal) -> bool {
    [
        &deal.north,
        &deal.east,
        &deal.south,
        &deal.west,
    ]
    .iter()
    .all(|h| h.len() == 13)
}

/// Solve the full 20-entry DD table for a complete deal.
pub fn solve_dd_table(deal: &Deal) -> DdTable {
    let hands = Hands::from_deal(deal);
    let total = hands.num_tricks() as u8;
    let mut tricks = [[0u8; 5]; 4];

    for strain in STRAINS {
        let trump = strain_trump(strain);
        // Fresh caches per trump, reused across the four declarer seats.
        let mut cutoff = CutoffCache::new(16);
        let mut pattern = PatternCache::new(16);

        for dir in DIRECTIONS {
            let declarer_seat = direction_to_seat(dir);
            let leader = (declarer_seat + 1) % 4; // defender to declarer's left
            let ns_tricks = Solver::new(hands, trump, leader)
                .solve_with_caches(&mut cutoff, &mut pattern);
            // Engine returns NS-pair tricks; convert to the declarer's side.
            let declarer_tricks = if matches!(dir, Direction::North | Direction::South) {
                ns_tricks
            } else {
                total - ns_tricks
            };
            tricks[dir_index(dir)][strain_index(strain)] = declarer_tricks;
        }
    }

    DdTable { tricks }
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
