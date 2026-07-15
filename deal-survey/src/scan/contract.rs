//! Effective-contract resolution for the baseline pass.
//!
//! Resolves the open spec question ("contract inference when `[Contract]` is
//! absent") with the proposed policy: use the explicit contract when present,
//! else the auction's final call when an auction exists, else none — never a
//! silent guess. DD contract analysis needs a declarer *seat*; if none can be
//! determined, we decline rather than attribute play to an unknown declarer.

use crate::model::ContractProvenance;
use bridge_types::{Board, Direction, Strain};

/// A contract resolved for DD analysis.
pub struct EffectiveContract {
    pub level: u8,
    pub strain: Strain,
    pub declarer: Direction,
    pub provenance: ContractProvenance,
    /// Human display, e.g. "4H by S".
    pub display: String,
}

/// Resolve the contract to analyze, or `None` if neither present nor inferable.
pub fn effective_contract(board: &Board) -> Option<EffectiveContract> {
    // 1. Explicit [Contract] (+ declarer from [Declarer] or the contract string).
    if let Some(cstr) = board.contract.as_deref() {
        if let Some(c) = bridge_types::Contract::parse(cstr) {
            if let Some(declarer) = board.declarer.or_else(|| Direction::from_char(c.declarer)) {
                return Some(EffectiveContract {
                    level: c.level,
                    strain: c.strain,
                    declarer,
                    provenance: ContractProvenance::Explicit,
                    display: format!("{} by {}", cstr, declarer.to_char()),
                });
            }
        }
    }

    // 2. Inferred from the auction's final contract.
    if let Some(auction) = board.auction.as_ref() {
        if let Some(fc) = auction.final_contract() {
            return Some(EffectiveContract {
                level: fc.level,
                strain: fc.strain,
                declarer: fc.declarer,
                provenance: ContractProvenance::Inferred,
                display: format!("{} by {}", fc.to_pbn(), fc.declarer.to_char()),
            });
        }
    }

    // 3. Neither present nor inferable — DD contract analysis is skipped.
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use bridge_types::{Auction, Board, Call, Strain};

    #[test]
    fn explicit_contract_is_used() {
        let board = Board::new()
            .with_contract("4H".to_string())
            .with_declarer(Direction::South);
        let ec = effective_contract(&board).expect("explicit contract");
        assert_eq!(ec.level, 4);
        assert_eq!(ec.strain, Strain::Hearts);
        assert_eq!(ec.declarer, Direction::South);
        assert_eq!(ec.provenance, ContractProvenance::Explicit);
    }

    #[test]
    fn falls_back_to_auction_final_call() {
        let mut auction = Auction::new(Direction::North);
        for call in [
            Call::bid(1, Strain::NoTrump),
            Call::Pass,
            Call::bid(3, Strain::NoTrump),
            Call::Pass,
            Call::Pass,
            Call::Pass,
        ] {
            auction.add_call(call);
        }
        let board = Board::new().with_auction(auction); // no [Contract]
        let ec = effective_contract(&board).expect("inferred from auction");
        assert_eq!(ec.level, 3);
        assert_eq!(ec.strain, Strain::NoTrump);
        assert_eq!(ec.provenance, ContractProvenance::Inferred);
    }

    #[test]
    fn no_contract_no_auction_declines() {
        assert!(effective_contract(&Board::new()).is_none());
    }
}
