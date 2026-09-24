# bridge-lesson-packaging

A shared, **collection-agnostic** toolset for packaging bridge lesson deals into **mixed-use
teaching materials** — for both **in-person** classes (the Full Table view: dealer files,
bidding sheets, dealer summaries, declarer's plans) and **online** lessons (the North-South
and South views, for platforms such as Shark Bridge).

Independent lesson collections (each with its own deals, taxonomy, provenance, and — where
applicable — its own content build) call this one toolset instead of maintaining separate,
drifting packaging scripts. The toolset knows nothing about any particular collection; a
collection supplies its deals and a small config, and gets the standard output.

## What it is / isn't

- **Is:** slicing lessons into board sets, rotating to Full Table / North-South / South views,
  block-replicating for multi-table play, rendering PDFs, and merging handouts — from PBN
  input, per `CONTRACT.md`.
- **Isn't:** deal authoring, hand generation/validation/repair, or any app/interactive
  product. Those stay in the collection that owns them.

## How a collection uses it

1. Arrange practice-deal PBNs by `category / lesson` (plus any optional per-lesson companion
   PDFs).
2. Provide a config (see `configs/example.conf`) — name prefix, set size(s), table views,
   replication, companion-doc globs, etc.
3. Run the packager over the collection's deals to produce the standard mixed-use materials tree.

See **`CONTRACT.md`** for the full output structure, the per-lesson auto-slicing rule, the
artifact list, and the optional `[SkillPath]` metadata pass-through.

## Reporting on a packaged tree

Two collection-agnostic tools read a finished materials tree and describe it:

| Tool | Output | Purpose |
|---|---|---|
| `stats.py` | `stats.json` + `stats.html` | Shape of the collection — lesson count, boards per lesson, distribution, build durations |
| `rotations_manifest.py` | `manifest.json` | Navigation index — category → lesson → set size → set number → table view, plus each lesson's intro PDF |

```bash
python3 rotations_manifest.py Rotations --name "My Collection"
```

`manifest.json` is what a client picker reads to offer the materials as choices: pick a
lesson, a board-set size, a set, and a table view, and it has the paths to that set's PBN,
PDF, bidding sheets, dealer summary, declarer's plan, block replication and merged handouts —
along with the lesson intro, which is what lets someone assemble a full class day from the
picker alone. Every path is relative to the tree root and POSIX-separated, so a web client
joins it to a base URL directly (~42 KB gzipped for a 50-lesson collection).

It is a *navigation* index only. Board identity — per-board tokens and stability — belongs to
the collection's own producer manifest, not here.

## Requirements

- [`bridge-wrangler`](https://github.com/bridge-craftwork/bridge-wrangler) — PBN rotation,
  block-replication, PBN→PDF.
- [`pdf-handouts`](https://github.com/bridge-craftwork/pdf-handouts) — PDF merge, headers/footers.

## Status

v1: `CONTRACT.md` + `package.sh` (the consolidated, config-driven builder) + `configs/*.conf` +
`stats.py` + `rotations_manifest.py`. `package.sh` was ported from the most feature-complete existing script and validated
to reproduce a collection's existing output structure byte-for-byte. Per-collection cutover
(pointing each repo at the shared tool and retiring its own script) is the remaining step.

## Why

The same materials are currently produced by several independent build scripts
(across collections and platforms) that have drifted apart. Consolidating on one contract and
one parameterized builder keeps every collection's teacher materials consistent and removes
the maintenance of parallel scripts.

## License

Released into the public domain under [The Unlicense](LICENSE).
