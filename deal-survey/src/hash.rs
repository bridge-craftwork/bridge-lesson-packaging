//! Content hash keying each deal record.
//!
//! CONTRACT ALIGNMENT (must-do before slice 2 records are trusted): the spec
//! requires this be the *same* hash as the deal-repository contract, so survey
//! records join against repository deals. That recipe (which fields participate,
//! rotation convention) has not been confirmed here yet. Until it is, this
//! hashes the deal in its canonical North-first `[Deal]` form (via
//! `Deal::to_pbn(North)`), which normalizes source whitespace/rotation — a
//! stable, deterministic placeholder. Swap `content_hash` for the contract's
//! recipe and bump nothing else; the hash is isolated behind this one function.

use sha2::{Digest, Sha256};

/// Hash of a deal's identity. See module note re: deal-repository contract.
pub fn content_hash(deal_pbn: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(deal_pbn.trim().as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
