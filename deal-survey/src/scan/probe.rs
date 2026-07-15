//! Stage 4 — technique probes (perturbation solves, gated on the baseline).
//!
//! Each probe perturbs the deal in a way that isolates one technique, re-solves
//! the contract double-dummy, and fires if the perturbation flips a making
//! contract to failing — i.e. making *required* that technique. DD does the
//! judging: if an alternative line still makes, the perturbation won't flip and
//! the probe won't fire.
//!
//! - `finesse`: swap a key honor between the defenders. If the contract now
//!   fails, its success depended on that honor's location.
//! - `ruff`: relocate the short trump hand's lowest trump to partner. If the
//!   contract now fails, a short-hand ruff was pulling its weight.
//!
//! Fired probes name the actual suit/honor (or trump) a human would — the
//! false-positive audit the spec calls for.

use crate::model::ProbeRecord;
use crate::scan::contract::EffectiveContract;
use crate::scan::dd;
use bridge_types::{Card, Deal, Direction, Hand, Rank, Strain, Suit};
use serde_json::json;

const SUITS: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];
const HONORS: [Rank; 4] = [Rank::Ace, Rank::King, Rank::Queen, Rank::Jack];

/// A perturbed deal to solve, carrying the evidence it would produce if it flips.
struct Perturbation {
    deal: Deal,
    evidence: serde_json::Value,
}

/// A perturbation with its solved declarer trick count.
struct Solved {
    evidence: serde_json::Value,
    tricks: u8,
}

/// Shared inputs for a probe on one deal/contract.
struct ProbeContext<'a> {
    deal: &'a Deal,
    ec: &'a EffectiveContract,
    required: u8,
    /// Declarer's DD tricks in the contract strain at baseline.
    baseline_tricks: u8,
    defenders: (Direction, Direction),
    partner: Direction,
}

trait Probe {
    fn name(&self) -> &'static str;
    fn perturb(&self, ctx: &ProbeContext) -> Vec<Perturbation>;
    fn verdict(&self, ctx: &ProbeContext, solved: &[Solved]) -> ProbeRecord;
}

/// Run all v1 probes; returns one record per probe. Perturbations are gathered
/// then solved (batched per probe) before verdicts.
pub fn run_probes(
    deal: &Deal,
    ec: &EffectiveContract,
    required: u8,
    baseline_tricks: u8,
) -> Vec<ProbeRecord> {
    let ctx = ProbeContext {
        deal,
        ec,
        required,
        baseline_tricks,
        defenders: defenders_of(ec.declarer),
        partner: partner_of(ec.declarer),
    };
    let probes: Vec<Box<dyn Probe>> = vec![Box::new(Finesse), Box::new(Ruff)];
    probes
        .iter()
        .map(|p| {
            let solved: Vec<Solved> = p
                .perturb(&ctx)
                .into_iter()
                .map(|pt| Solved {
                    tricks: dd::solve_contract(&pt.deal, ec.strain, ec.declarer),
                    evidence: pt.evidence,
                })
                .collect();
            p.verdict(&ctx, &solved)
        })
        .collect()
}

// --- finesse ---------------------------------------------------------------

struct Finesse;

impl Probe for Finesse {
    fn name(&self) -> &'static str {
        "finesse"
    }

    fn perturb(&self, ctx: &ProbeContext) -> Vec<Perturbation> {
        let (d0, d1) = ctx.defenders;
        let decl = ctx.deal.hand(ctx.ec.declarer);
        let dummy = ctx.deal.hand(ctx.partner);
        let mut out = Vec::new();

        for suit in SUITS {
            let c0 = ctx.deal.hand(d0).cards_in_suit(suit);
            let c1 = ctx.deal.hand(d1).cards_in_suit(suit);
            // Both defenders must hold the suit for the honor's location to be
            // swappable and genuinely ambiguous.
            if c0.is_empty() || c1.is_empty() {
                continue;
            }
            for honor in HONORS {
                let hr = honor as u8;
                let in0 = c0.iter().any(|c| c.rank as u8 == hr);
                let in1 = c1.iter().any(|c| c.rank as u8 == hr);
                if in0 == in1 {
                    continue; // need exactly one defender to hold it
                }
                // A finesse is only conceivable if declarer's side holds a card
                // ranked above the honor in that suit (a tenace to lead toward).
                let above = decl
                    .cards_in_suit(suit)
                    .iter()
                    .chain(dummy.cards_in_suit(suit).iter())
                    .any(|c| c.rank as u8 > hr);
                if !above {
                    continue;
                }
                let (holder, other, other_cards) = if in0 { (d0, d1, &c0) } else { (d1, d0, &c1) };
                let _ = other_cards; // holder's cards not needed; use other's below
                let low = *lowest(&ctx.deal.hand(other).cards_in_suit(suit));
                let deal = swap(ctx.deal, holder, Card::new(suit, honor), other, low);
                out.push(Perturbation {
                    deal,
                    evidence: json!({ "suit": suit.to_char().to_string(), "honor": honor.to_char().to_string() }),
                });
            }
        }
        out
    }

    fn verdict(&self, ctx: &ProbeContext, solved: &[Solved]) -> ProbeRecord {
        verdict_on_flip(self.name(), ctx, solved)
    }
}

// --- ruff ------------------------------------------------------------------

struct Ruff;

impl Probe for Ruff {
    fn name(&self) -> &'static str {
        "ruff"
    }

    fn perturb(&self, ctx: &ProbeContext) -> Vec<Perturbation> {
        let Some(trump) = strain_suit(ctx.ec.strain) else {
            return vec![]; // ruffing needs a trump suit
        };
        let decl = ctx.ec.declarer;
        let partner = ctx.partner;
        let dl = ctx.deal.hand(decl).suit_length(trump);
        let pl = ctx.deal.hand(partner).suit_length(trump);
        // Ruffing tricks come from the SHORT trump hand; relocate its lowest
        // trump to the long hand (partner) and see if the contract still makes.
        let (short, long) = if dl <= pl { (decl, partner) } else { (partner, decl) };
        let short_trumps = ctx.deal.hand(short).cards_in_suit(trump);
        if short_trumps.is_empty() {
            return vec![];
        }
        let long_side: Vec<Card> = ctx
            .deal
            .hand(long)
            .cards()
            .iter()
            .filter(|c| c.suit != trump)
            .copied()
            .collect();
        if long_side.is_empty() {
            return vec![];
        }
        let deal = swap(
            ctx.deal,
            short,
            *lowest(&short_trumps),
            long,
            *lowest(&long_side),
        );
        vec![Perturbation {
            deal,
            evidence: json!({ "trump": trump.to_char().to_string(), "detail": "relocated short-hand trump to partner" }),
        }]
    }

    fn verdict(&self, ctx: &ProbeContext, solved: &[Solved]) -> ProbeRecord {
        verdict_on_flip(self.name(), ctx, solved)
    }
}

// --- shared ----------------------------------------------------------------

/// A probe fires if any perturbation flips a making contract to failing.
fn verdict_on_flip(name: &str, ctx: &ProbeContext, solved: &[Solved]) -> ProbeRecord {
    let makes = ctx.baseline_tricks >= ctx.required;
    let flips: Vec<serde_json::Value> = solved
        .iter()
        .filter(|s| makes && s.tricks < ctx.required)
        .map(|s| {
            let mut e = s.evidence.clone();
            e["baseline"] = json!(ctx.baseline_tricks);
            e["perturbed"] = json!(s.tricks);
            e
        })
        .collect();
    let fired = !flips.is_empty();
    ProbeRecord {
        name: name.to_string(),
        fired,
        evidence: if fired { json!({ "flips": flips }) } else { json!({}) },
    }
}

fn swap(deal: &Deal, da: Direction, ca: Card, db: Direction, cb: Card) -> Deal {
    let mut d = deal.clone();
    let mut ha: Vec<Card> = d.hand(da).cards().to_vec();
    remove_one(&mut ha, ca);
    ha.push(cb);
    let mut hb: Vec<Card> = d.hand(db).cards().to_vec();
    remove_one(&mut hb, cb);
    hb.push(ca);
    d.set_hand(da, Hand::from_cards(ha));
    d.set_hand(db, Hand::from_cards(hb));
    d
}

fn remove_one(v: &mut Vec<Card>, c: Card) {
    if let Some(i) = v.iter().position(|x| *x == c) {
        v.remove(i);
    }
}

fn lowest(cards: &[Card]) -> &Card {
    cards.iter().min_by_key(|c| c.rank as u8).expect("non-empty")
}

fn partner_of(d: Direction) -> Direction {
    match d {
        Direction::North => Direction::South,
        Direction::South => Direction::North,
        Direction::East => Direction::West,
        Direction::West => Direction::East,
    }
}

fn defenders_of(d: Direction) -> (Direction, Direction) {
    match d {
        Direction::North | Direction::South => (Direction::East, Direction::West),
        Direction::East | Direction::West => (Direction::North, Direction::South),
    }
}

fn strain_suit(strain: Strain) -> Option<Suit> {
    match strain {
        Strain::Clubs => Some(Suit::Clubs),
        Strain::Diamonds => Some(Suit::Diamonds),
        Strain::Hearts => Some(Suit::Hearts),
        Strain::Spades => Some(Suit::Spades),
        Strain::NoTrump => None,
    }
}
