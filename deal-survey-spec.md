# Spec: `deal-survey` — Lesson Collection Characterization Tool

## Summary

A Rust CLI that walks a directory of PBN lesson collections and produces, for each deal, a machine-readable characterization record (structural facts, double-dummy baseline, cardplay difficulty, detected techniques), then rolls those records up into a per-collection profile. The goal is to catalog the strengths and weaknesses of each lesson collection (e.g., Baker Bridge: wide auction variety, cardplay mostly "cash your winners") and to define measurable lesson ideals per class context.

Lives in `bridge-lesson-packaging` alongside the existing packaging scripts, as characterization is a natural front-end to packaging decisions.

## Motivation

- Multiple third-party and in-house collections exist with informally-known but undocumented difficulty and coverage profiles.
- Class placement (SC Morning vs. SC Afternoon/2-over-1) needs deals whose cardplay difficulty matches the bidding lesson — a bidding lesson should not be contaminated by play failures.
- Deal generation with David benefits from a shared, inspectable difficulty methodology rather than per-person judgment.
- No public tool does this; existing ecosystem stops at DD trick counts (BridgeComposer, endplay, DealMasterPro classification-by-shape).

## Non-goals (v1)

- Bidding-difficulty scoring beyond cheap structural proxies (auction length, presence of alerts/conventions).
- Defense-difficulty scoring (lead sensitivity is noted as a future probe).
- Single-dummy / percentage-line analysis. All play scoring is DD-based with perturbation probes.
- Any modification of source PBN files. The tool is read-only over collections.

## Architecture

Depends on the existing in-house crates:

- PBN parsing: existing Rust PBN library
- DD solving: existing Rust DD solver library

Pipeline stages, each independently runnable:

1. **Structural pass** (no solves): parse each deal, record what the PBN actually contains — deal, dealer, vul, contract/declarer present or absent, auction present, play section present, commentary present (inline `{}` vs `%` lines vs custom tags), custom tag inventory (e.g., bridge-mastery tags). Deals without a designated contract are flagged `contract: inferred` or `contract: none` — never silently guessed.
2. **Baseline DD pass** (1 full solve per deal): 20-entry DD table, par score/contracts, and derived facts for the designated contract: DD makeable (Y/N), slack (DD tricks minus tricks required), sensitivity to declarer seat.
3. **Cash-out check** (no extra solves): immediate top winners for declarer's side in the contract strain vs. tricks required. If winners >= required, cardplay difficulty = 0.
4. **Probe pass** (perturbation solves, gated on the baseline): only runs on deals not resolved at difficulty 0. Each probe is an implementation of a `Probe` trait:
   - `name() -> &str`
   - `perturb(deal, contract) -> Vec<PerturbedDeal>`
   - `verdict(baseline, perturbed_results) -> ProbeResult { fired: bool, evidence }`

   v1 probes:
   - `finesse` — swap candidate key honors between defenders; fires if DD result in the contract strain flips.
   - `ruff` — remove/relocate a short-hand trump; fires if DD result drops.

   Perturbed deals are accumulated and solved in batches (`SolveAllBoards`-style) rather than one at a time.
5. **Roll-up**: fold per-deal records into a collection profile (difficulty histogram, technique coverage, structural coverage, editorial metadata).

## Difficulty ladder (cardplay)

| Level | Meaning | Detection |
|---|---|---|
| 0 | Cash-out: top tricks suffice | winners >= required |
| 1 | Establish/drive out, no entry or timing problem | delta 1–2, no probe fires |
| 2 | Single technique required (finesse or ruff) | exactly one probe fires |
| 3 | Timing/entries/hold-up | reserved — future probes |
| 4 | DD-only / advanced (endplay, squeeze, or no natural line) | DD makes but heuristics fail — future |

v1 assigns 0, 1, 2, or `unclassified` (probe evidence ambiguous or multiple probes fire). `unclassified` is an honest output, not an error.

## Per-deal record (JSON)

One JSON object per deal, keyed by content hash (same hash as the deal-repository contract).

```json
{
  "hash": "…",
  "source": { "collection": "baker-bridge", "file": "…", "board": 7 },
  "structural": {
    "contract": "4H by S", "contract_provenance": "explicit",
    "auction": true, "play": false,
    "commentary": { "present": true, "style": "inline" },
    "custom_tags": ["BridgeMastery"]
  },
  "baseline": {
    "dd_table": { "...": "20 entries" },
    "par": "…",
    "contract_dd_makes": true,
    "slack": 1
  },
  "cardplay": {
    "immediate_winners": 10,
    "required": 10,
    "difficulty": 0
  },
  "probes": [
    { "name": "finesse", "fired": false, "evidence": {} }
  ],
  "versions": { "tool": "0.1.0", "ladder": 1 }
}
```

Records are the debuggable ledger; the collection profile is derived and never hand-edited. `versions` supports cache invalidation when the ladder or probe logic changes.

## Collection profile (JSON + human-readable report)

- Difficulty histogram (counts and fractions at each ladder level)
- Structural coverage: % with auction, % with commentary, % with explicit contract, tag density
- Contract mix: strain × level distribution, declarer seat distribution
- Technique coverage matrix (which probes fire, how often)
- Editorial metadata block (manually supplied per collection, in a small TOML sidecar): licensing, commentary quality notes, intended audience

## CLI

```
deal-survey scan <collection-dir> --out records/       # stages 1–4, one JSON per deal
deal-survey profile records/ --out profiles/           # stage 5
deal-survey report profiles/                           # human-readable summary table
```

- Caching: skip deals whose (hash, tool version, ladder version) record already exists.
- All output deterministic and diffable.

## Validation plan

Ground truth: Rick's existing informal grading of the collections (e.g., Baker Bridge ≈ level 0 cardplay). Acceptance for v1:

1. **Collection-level sanity**: the difficulty histogram rank-orders known collections correctly (Baker at the easy end, known play-focused sets harder).
2. **Deal-level spot check**: a hand-classified calibration set of ~12–20 deals spanning levels 0–2; probe verdicts must match on ≥ 90% with no silent misclassification (disagreements land in `unclassified`, not the wrong level).
3. **False-positive audit**: for every fired probe on the calibration set, the evidence block must identify the actual suit/honor a human would name.

Calibration set lives in the repo under `calibration/` with expected records checked in as fixtures — same ledger-test discipline as the grid arranger work.

## Milestones (roadmap slices)

1. **survey-structural** — parser walk + structural records + report. No solver dependency. Answers "what does each collection actually contain."
2. **survey-baseline** — DD table, par, slack, cash-out check. Answers "what fraction of Baker is difficulty 0" quantitatively.
3. **survey-probes** — probe trait, finesse + ruff probes, batched perturbation solves, calibration fixtures.
4. **survey-profile** — roll-up, editorial sidecar, report command.
5. *(later)* entry/hold-up/timing probes; lead-sensitivity defense axis; bidding decision-point proxies.

## Open questions

- Contract inference policy when `[Contract]` is absent: skip, use par, or use final auction call? (Proposal: use final auction call when an auction exists, else flag and skip DD contract analysis.)
- Does this become a bilateral contract with `Practice-Bidding-Scenarios` (i.e., David's generator consumes difficulty records)? If yes, the per-deal record schema needs an ADR before slice 3.
- Where do editorial sidecars live — with the collections or in this repo?
