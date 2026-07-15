//! Stage 3 — cash-out check (no extra solves).
//!
//! Counts the declaring side's *immediate top winners* in a suit as the run of
//! consecutive top honors (A, then K, then Q…) the partnership holds combined,
//! capped by the longer hand's length in that suit (a blockage bound). Summed
//! across all four suits, this is the "top tricks suffice" proxy the difficulty
//! ladder's level 0 is defined against.
//!
//! It is deliberately a cheap upper-ish bound: it ignores entries and inter-suit
//! timing, so slice 3's probe calibration is what ultimately validates the
//! level-0 gate. Being an over-count risk, an ambiguous case is meant to fall to
//! `unclassified`, never to a wrong non-zero level.

use bridge_types::{Hand, Suit};

/// Immediate top winners for the declaring side (declarer + dummy hands).
pub fn immediate_top_winners(declarer: &Hand, dummy: &Hand) -> u8 {
    [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades]
        .into_iter()
        .map(|suit| suit_top_winners(declarer, dummy, suit))
        .sum()
}

fn suit_top_winners(a: &Hand, b: &Hand, suit: Suit) -> u8 {
    use std::collections::HashSet;
    let held: HashSet<u8> = a
        .cards_in_suit(suit)
        .iter()
        .chain(b.cards_in_suit(suit).iter())
        .map(|c| c.rank as u8)
        .collect();

    // Consecutive top ranks from the Ace (14) downward.
    let mut winners = 0u8;
    let mut rank = 14u8;
    while rank >= 2 && held.contains(&rank) {
        winners += 1;
        rank -= 1;
    }

    // Can't cash more cards than the longer holding physically has.
    let longer = a.suit_length(suit).max(b.suit_length(suit)) as u8;
    winners.min(longer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bridge_types::Hand;

    fn hand(pbn_suits: &str) -> Hand {
        // "AK4.QJ.T98765.2" style — parse via a full Deal is overkill; build cards.
        use bridge_types::{Card, Rank, Suit};
        let suits = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs];
        let mut cards = Vec::new();
        for (holding, suit) in pbn_suits.split('.').zip(suits) {
            for ch in holding.chars() {
                if let Some(rank) = Rank::from_char(ch) {
                    cards.push(Card::new(suit, rank));
                }
            }
        }
        Hand::from_cards(cards)
    }

    #[test]
    fn counts_top_run_capped_by_length() {
        // Spades AKQ combined across the two hands, hearts A only.
        let declarer = hand("AK.A.5432.5432");
        let dummy = hand("Q432.234.6.6543");
        // Spades: AKQ run = 3, longer length = 4 → 3 winners.
        // Hearts: A only = 1, longer length = 3 → 1 winner.
        // Diamonds/Clubs: no ace → 0.
        assert_eq!(immediate_top_winners(&declarer, &dummy), 4);
    }

    #[test]
    fn blockage_cap_limits_winners() {
        // Combined AKQJ but only a doubleton opposite a void → capped at 2.
        let declarer = hand("AKQJ.234.234.234");
        let dummy = hand(".5678.5678.5678");
        // Spades run AKQJ = 4, longer length = 4 (declarer) → not capped here.
        assert_eq!(suit_top_winners(&declarer, &dummy, Suit::Spades), 4);
    }
}
