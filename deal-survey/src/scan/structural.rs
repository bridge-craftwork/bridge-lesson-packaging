//! Stage 1 — structural pass. No solves.
//!
//! Now a thin mapping over the shared `bridge_encodings` parser: it populates a
//! `bridge_types::Board` with contract/declarer/auction/play/commentary and an
//! `extra_tags` inventory of supplemental PBN tags, so this stage just reads
//! those off rather than re-scanning raw text.

use crate::model::{strain_index, AuctionInfo, Commentary, ContractProvenance, Source, Structural};
use bridge_types::{Auction, Board, Call, Direction, Strain};

/// Standard PBN tags that carry no dedicated `Board` field and so land in
/// `extra_tags`, but are *not* collection-specific — excluded from the custom
/// tag inventory so it reflects genuine authoring tags (SkillPath, Difficulty…).
const NON_CUSTOM_EXTRA_TAGS: &[&str] = &[
    "Scoring", "Room", "Round", "Score", "Generator", "Annotator",
    "OptimumResultTable", "Table", "Stage", "Section", "HomeTeam", "VisitTeam",
];

/// Derive the stage-1 structural record from a parsed board.
pub fn structural_of(board: &Board) -> Structural {
    let provenance = if board.contract.is_some() {
        ContractProvenance::Explicit
    } else {
        // Auction-based inference is a slice-2 policy decision; until then an
        // absent contract is honestly `none`, never guessed.
        ContractProvenance::None
    };

    let mut custom_tags: Vec<String> = board
        .extra_tags
        .iter()
        .map(|(name, _)| name.clone())
        .filter(|name| !NON_CUSTOM_EXTRA_TAGS.contains(&name.as_str()))
        .collect();
    custom_tags.dedup();

    let present = !board.commentary.is_empty();
    Structural {
        contract: board.contract.clone(),
        contract_provenance: provenance,
        auction: auction_info(board.auction.as_ref()),
        play: board.play.is_some(),
        commentary: Commentary {
            present,
            // The reader captures `{...}` blocks; PBN `%`/`;` lines are file
            // directives, not board commentary — so present commentary is inline.
            style: present.then(|| "inline".to_string()),
        },
        custom_tags,
    }
}

/// Cheap auction-complexity proxies from the parsed auction.
fn auction_info(auction: Option<&Auction>) -> AuctionInfo {
    let Some(a) = auction else {
        return AuctionInfo::absent();
    };
    let (mut bids, mut doubles, mut high_bids) = (0u8, 0u8, 0u8);
    let (mut ns_bid, mut ew_bid) = (false, false);
    for (i, ac) in a.calls.iter().enumerate() {
        match ac.call {
            Call::Bid { level, strain } => {
                bids += 1;
                if matches!(a.caller(i), Direction::North | Direction::South) {
                    ns_bid = true;
                } else {
                    ew_bid = true;
                }
                if is_high_bid(level, strain) {
                    high_bids += 1;
                }
            }
            Call::Double | Call::Redouble => doubles += 1,
            _ => {}
        }
    }
    let double_of_final = a
        .final_contract()
        .map(|fc| fc.doubled || fc.redoubled)
        .unwrap_or(false);
    AuctionInfo {
        present: true,
        bids,
        contested: ns_bid && ew_bid,
        doubles,
        double_of_final,
        high_bids,
    }
}

/// A contract bid above 3NT, excluding the normal games 4H/4S/5C/5D.
fn is_high_bid(level: u8, strain: Strain) -> bool {
    let rank = (level as i32 - 1) * 5 + strain_index(strain) as i32;
    let three_nt = 2 * 5 + strain_index(Strain::NoTrump) as i32;
    if rank <= three_nt {
        return false;
    }
    !matches!(
        (level, strain),
        (4, Strain::Hearts) | (4, Strain::Spades) | (5, Strain::Clubs) | (5, Strain::Diamonds)
    )
}

/// Build a `Source` for a board within a collection scan.
pub fn source_for(collection: &str, file: &str, board: &Board) -> Source {
    Source {
        collection: collection.to_string(),
        file: file.to_string(),
        board: board.number,
        category: category_of(board, file),
    }
}

/// Lesson grouping for a deal: the `[SkillPath]` first component (Baker-style),
/// else the leading folder of the file path (folder-structured collections),
/// else "(uncategorized)".
fn category_of(board: &Board, file: &str) -> String {
    if let Some(sp) = board.extra_tag("SkillPath") {
        if let Some(cat) = sp.split('/').next() {
            if !cat.is_empty() {
                return cat.to_string();
            }
        }
    }
    if let Some((dir, _)) = file.split_once('/') {
        return dir.to_string();
    }
    "(uncategorized)".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bridge_types::Board;

    #[test]
    fn maps_contract_sections_and_filters_custom_tags() {
        let board = Board::new()
            .with_contract("4S".to_string())
            .with_extra_tag("SkillPath", "majors/weak-two")
            .with_extra_tag("Scoring", "IMP") // standard-but-unmodeled → not custom
            .with_commentary("Draw trumps.".to_string());

        let s = structural_of(&board);
        assert_eq!(s.contract.as_deref(), Some("4S"));
        assert_eq!(s.contract_provenance, ContractProvenance::Explicit);
        assert!(!s.auction.present); // none attached
        assert!(s.commentary.present);
        assert_eq!(s.commentary.style.as_deref(), Some("inline"));
        assert_eq!(s.custom_tags, vec!["SkillPath".to_string()]);
    }

    #[test]
    fn absent_contract_is_none_not_guessed() {
        let s = structural_of(&Board::new());
        assert_eq!(s.contract_provenance, ContractProvenance::None);
        assert!(s.contract.is_none());
    }
}
