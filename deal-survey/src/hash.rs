//! Content hash keying each deal record.
//!
//! ROTATION-CANONICAL: a deal and any of its table rotations are the same
//! physical board (identical cards, identical cardplay difficulty — a rotation
//! just relabels the compass). Packaged lesson sets exploit this to seat the
//! student consistently (and to present "standard/nonstandard" variants), so the
//! *same* board recurs under different seat assignments. Hashing the deal as
//! written would treat those as distinct; instead we hash the lexicographically
//! smallest of the four rotations, so rotated duplicates collapse to one record.
//!
//! (This is the deal-repository-contract alignment flagged since slice 1; the
//! rotation normalization is the substantive part of that recipe.)

use bridge_types::{Deal, Direction};
use sha2::{Digest, Sha256};

const SEATS: [Direction; 4] = [
    Direction::North,
    Direction::East,
    Direction::South,
    Direction::West,
];

/// Rotation-invariant hash of a deal's identity.
pub fn content_hash(deal: &Deal) -> String {
    let canonical = (0..4)
        .map(|k| rotate(deal, k).to_pbn(Direction::North))
        .min()
        .expect("four rotations");
    let digest = Sha256::digest(canonical.as_bytes());
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Rotate the table `k` seats clockwise (N→E→S→W).
fn rotate(deal: &Deal, k: usize) -> Deal {
    let mut d = Deal::default();
    for (i, &seat) in SEATS.iter().enumerate() {
        let src = SEATS[(i + 4 - k) % 4];
        d.set_hand(seat, deal.hand(src).clone());
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotations_hash_identically() {
        let deal =
            Deal::from_pbn("S:94.QJ76.KQ65.AJ6 QJ3.432.A873.T42 A76.A98.JT9.KQ85 KT852.KT5.42.973")
                .unwrap();
        // The same board rotated one seat (the "nonstandard" presentation).
        let rotated =
            Deal::from_pbn("N:QJ3.432.A873.T42 A76.A98.JT9.KQ85 KT852.KT5.42.973 94.QJ76.KQ65.AJ6")
                .unwrap();
        assert_eq!(content_hash(&deal), content_hash(&rotated));
    }

    #[test]
    fn distinct_deals_hash_differently() {
        let a =
            Deal::from_pbn("N:K843.T542.J6.863 AQJ7.K.Q75.AT942 962.AJ7.KT82.J75 T5.Q9863.A943.KQ")
                .unwrap();
        // b swaps North and East only — a reflection, not a rotation.
        let b =
            Deal::from_pbn("N:AQJ7.K.Q75.AT942 K843.T542.J6.863 962.AJ7.KT82.J75 T5.Q9863.A943.KQ")
                .unwrap();
        assert_ne!(content_hash(&a), content_hash(&b));
    }
}
