# Lesson materials packaging contract (v1)

This defines the **standard structure and artifacts** for the teaching materials a bridge
lesson collection produces, and the **configuration a collection supplies** to the shared
packaging toolset.

The materials are **mixed-use** — packaged for two delivery modes:

- **In-person (at the table):** the **Full Table** view — all four hands, physically dealt —
  plus its dealer files, bidding sheets, dealer summaries, and declarer's plans.
- **Online:** the **North-South** and **South** views — reduced-seat files for online play
  platforms (e.g. Shark Bridge), where a student sees only their own seat(s).

A collection chooses which views to emit, so a collection can target in-person only, online
only, or both.

The toolset is **collection-agnostic**: it knows nothing about any particular collection. A
collection supplies (a) its deals as PBN files organized by a taxonomy, and (b) a config; the
toolset produces the standard output below. This lets independent collections share one
packaging pipeline instead of maintaining divergent build scripts.

## Inputs

A collection provides:

1. **Deals** — one PBN file of practice deals per lesson, arranged in a taxonomy of
   `category / lesson` folders (or an equivalent map from lesson → category).
2. **Companion documents** *(optional)* — zero or more per-lesson PDFs (lesson plans,
   exercises, introductions) to fold into handouts and/or ship alongside.
3. **A config** — the parameters below.

The toolset does not generate or validate deals; deal authoring, hand generation, and any
app-specific processing belong to the collection, not here.

## Configuration parameters

| Param | Meaning | Default |
|---|---|---|
| `namePrefix` | string prefixed to every output filename | *(empty)* |
| `categoryNumbering` | prefix categories with an order number (`1. `, `2. `…) | off |
| `setSizes` | board-set slice size(s), e.g. `[4,5,6]` or `[6]` | `[6]` |
| `tables` | which table views to emit | `[FullTable, NorthSouth, South]` |
| `replicateTables` | copies of a set for multi-table play (the `T` in `KxT`) | none |
| `companionDocs` | glob(s) selecting per-lesson companion PDFs | *(none)* |
| `mergeComponents` | group per-view PDFs under `Components/` and merge into one Handouts PDF | on |
| `declarerPlan` | emit declarer's-plan sheet(s); layouts: `2up`, `1up` | `2up` |
| `lin` | also emit LIN files for online play | off |
| `studentSeat` | the single-student seat for the `South` view | `S` |

**No collection identity, source, license, or repo name is part of this config** — those are
the collection's concern, not the toolset's.

## Design principle: per-lesson slicing, automatic by size

A "set" is what one table plays in a session (≈4–9 boards). Slicing is decided **per lesson,
from its own board count** — there is no global "slice / don't slice" switch:

> **For each declared set size S, a lesson of B boards is emitted as ⌈B / S⌉ sets — but only
> when B > S. A lesson with B ≤ S is a single, unsliced set.**

One rule scales to any collection: a collection of large lessons slices into many sets; a
collection of small lessons (every lesson ≤ S) emits one set each and never multiplies. No
per-collection special-casing, and no wasted "Set 1" wrapper on small lessons.

## Standard per-lesson output

For a lesson of *B* boards:

```
{Lesson}/
  {prefix}{Lesson}.pbn                     full lesson (all boards)
  {prefix}{Lesson} {companion}             companion PDF(s), if any
  <for each table view in `tables`:>       Full Table / North-South / South
    {prefix}{Lesson} <set> <view>.pbn/.pdf
    {prefix}{Lesson} <set> - {K}x{T}.pbn/.pdf   block-replicated for T tables (if replicateTables)
    [Components/]                          per-view component PDFs (if mergeComponents)
      N. {prefix}{Lesson} <component>.pdf
    {prefix}{Lesson} <set> Handouts <view>.pdf   merged from Components
  <Full Table view only:>
    {prefix}{Lesson} <set> Bidding Sheets.pdf
    {prefix}{Lesson} <set> Dealer Summary.pdf     non-standard dealer / vulnerability sheet
    {prefix}{Lesson} <set> Declarer's Plan[ {layout}].pdf
```

- **`<set>`** = `Set N (K hands)` when the lesson is sliced; a single-set lesson omits the set
  number (e.g. `(B hands)` or a plain `practice deals` label per `namePrefix` style).
- When `setSizes` has more than one size, the lesson is emitted once per size, each under its
  own `{S}-Board Sets/` folder.

### Table views

| View | Hands shown | Delivery mode | Use |
|---|---|---|---|
| **Full Table** (NESW) | all four | **in-person** | physically dealt; dealer file / reference |
| **North-South** (NS) | the partnership | **online** | partnership practice on an online platform |
| **South** (`studentSeat`) | one seat | **online** | single-student practice on an online platform |

`tables` selects which of these a collection emits (in-person only, online only, or both).

### Standard artifacts

| Artifact | Required? | Scope |
|---|---|---|
| full-lesson PBN | required | lesson |
| per-view PBN + PDF | required | each set × view |
| block-replication `{K}x{T}` | if `replicateTables` | each set |
| merged Handouts PDF | required | each view |
| Bidding Sheets | required | Full Table |
| Dealer Summary | required | Full Table |
| Declarer's Plan | optional (`declarerPlan`) | Full Table |
| companion doc(s) | optional (`companionDocs`) | lesson |
| LIN | optional (`lin`) | each view |

## Optional metadata (pass-through)

A collection's PBNs **may** carry `[SkillPath "category/skill"]` (a hierarchical skill
classification). The toolset **passes it through unchanged** into the packaged PBNs so that
lesson material stays searchable/filterable across collections. It is **not required** and the
toolset never invents it. Any other collection-specific PBN tags are likewise passed through
untouched; the toolset strips only interactive/app control tags that have no meaning on paper
(a documented, fixed list).

## Non-goals

- No deal generation, validation, or repair.
- No app/interactive artifacts (control directives, board-identity tokens, app manifests) —
  those belong to a collection that also ships an app product.
- No knowledge of collection identity, provenance, licensing, or hosting.

## Tools

Built on `bridge-wrangler` (PBN rotation, block-replication, PBN→PDF) and `pdf-handouts` (PDF
merge + headers/footers). See `README.md` for invocation and `configs/example.conf` for a
worked config.

## Open questions (v1 → v2)

1. Config format (TOML/JSON/env) and whether it lives in the collection repo or is passed on
   the command line.
2. `manifest.json` per collection (lesson → category, board count, set sizes, artifact paths)
   for completeness verification. Deferred; the directory structure is the v1 contract.
3. Exact component ordering + which components are merged into Handouts vs shipped standalone.
4. Standard set-label wording for single-set vs sliced lessons.
