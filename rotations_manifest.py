#!/usr/bin/env python3
"""Emit a manifest describing a packaged lesson-materials tree.

Collection-agnostic, like stats.py: point it at a packaged output tree (the standard
`{category}/{lesson}/...` structure from CONTRACT.md) and it writes a JSON index of
everything that tree holds -- categories, lessons, board-set sizes, individual sets,
table views, and the per-artifact files that belong to each.

It is a *navigation* index, not a board-identity record. Board identity (per-board
tokens, stability) belongs to the collection's own producer manifest; this describes
the shape of the packaged materials so a client can offer them as choices:

    category -> lesson -> set size (4/5/6) -> set number -> table view

plus the lesson's companion intro PDF, which is what lets someone assemble a full
lesson set for a class day from the picker alone.

A lesson that fits in one set is packaged flat (its views sit directly in the lesson
folder): it has no `setSizes`, and `all` carries the set's full artifacts.

Schema:

    {
      "schemaVersion": 1,
      "kind": "lesson-materials",
      "name": "<collection display name>",
      "generatedAt": "<ISO 8601 — see below>",
      "contentHash": "<16 hex chars over the described content>",
      "views": ["Full Table", "North-South", "South"],
      "categories": [
        {"name": "2. Bidding Conventions",
         "lessons": [
           {"name": "Ogust after Weak 2-Bid",
            "title": "Baker Bridge Ogust",       # filename stem shared by the lesson's files
            "path": "2. Bidding Conventions/Ogust after Weak 2-Bid",
            "boards": 10,
            "intro": "<relative path>|null",
            "lessonPbn": "<relative path>|null",
            "documents": ["<relative path>", ...],   # other lesson-root PDFs; only when any
            "all": {"boards": 10, "views": {"Full Table": {...}, ...}},
            "setSizes": [
              {"size": 4,
               "sets": [
                 {"set": 1, "boards": 4,
                  "views": {
                    "Full Table": {"pbn": "...", "pdf": "...", "biddingSheets": "...",
                                   "dealerSummary": "...", "declarersPlan": "...",
                                   "handouts": "...",
                                   "replicated": {"label": "4x9", "pbn": "...", "pdf": "..."}},
                    "North-South": {"pbn": "...", "pdf": "..."},
                    "South": {"pbn": "...", "pdf": "..."}}}]}]}]}]
    }

Every path is relative to the tree root and POSIX-separated, so a web client can join
it to a base URL directly.

`generatedAt` is supplied by the caller and is typically pinned to the collection's
source data rather than the build clock, so that rebuilding unchanged content produces
an unchanged file. That makes it a *version* of the source, not a freshness signal:
it does not move when packaging changes. Use `contentHash` for freshness and cache
busting -- it is a digest of the described content, so it changes if and only if
something in the tree changed, and is identical across rebuilds of the same tree.

Usage:
    python3 rotations_manifest.py <tree_root> [--name NAME] [--generated-at ISO8601]
        [--out manifest.json]
"""
import argparse
import datetime
import hashlib
import json
import os
import re
import sys

VIEWS = ["Full Table", "North-South", "South"]

# "... Set 3 (4 hands) ..." -- the set number and how many boards it actually holds.
SET_RE = re.compile(r"\bSet (\d+)\b")
HANDS_RE = re.compile(r"\((\d+) hands?\)")
# Block replication, e.g. "Set 1 - 4x9" = 4 boards across 9 tables.
REPLICATED_RE = re.compile(r" - (\d+x\d+)$")
# A set file's name less its set/hands/view suffix: the lesson PBN it was cut from.
SET_FILE_STEM_RE = re.compile(r"( Set \d+)? \(\d+ hands?\).*$")
ALL_DIR_RE = re.compile(r"^All (\d+) boards$")
SET_DIR_RE = re.compile(r"^(\d+)-Board Sets$")
BOARD_RE = re.compile(r"^\[Board ", re.M)


def rel(root, path):
    return os.path.relpath(path, root).replace(os.sep, "/")


def count_boards(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            return len(BOARD_RE.findall(f.read()))
    except OSError:
        return None


def classify(stem, view):
    """Which artifact a file is, from its stem. Returns a manifest key, or None for the
    set's own PBN/PDF (handled by the caller)."""
    if stem.endswith(" Bidding Sheets"):
        return "biddingSheets"
    if stem.endswith(" Dealer Summary"):
        return "dealerSummary"
    if stem.endswith(" Declarers Plan"):
        return "declarersPlan"
    if stem.endswith(" Handouts") or stem.endswith(" Handouts " + view):
        return "handouts"
    return None


def scan_view_dir(root, view_dir, view):
    """Group a view folder's files by set number.

    Returns {set_number: {"boards": n, "pbn": ..., "pdf": ..., <artifact>: ...}}. A view
    folder holding a single unsliced set uses set number 0.
    """
    sets = {}
    try:
        entries = sorted(os.listdir(view_dir))
    except OSError:
        return sets
    for name in entries:
        path = os.path.join(view_dir, name)
        if not os.path.isfile(path):
            continue
        stem, ext = os.path.splitext(name)
        if ext.lower() not in (".pbn", ".pdf"):
            continue
        m = SET_RE.search(stem)
        num = int(m.group(1)) if m else 0
        entry = sets.setdefault(num, {"set": num, "boards": None})

        replicated = REPLICATED_RE.search(stem)
        if replicated:
            r = entry.setdefault("replicated", {"label": replicated.group(1)})
            r["pbn" if ext.lower() == ".pbn" else "pdf"] = rel(root, path)
            continue

        kind = classify(stem, view)
        if kind:
            entry[kind] = rel(root, path)
            continue

        # The set's own file.
        entry["pbn" if ext.lower() == ".pbn" else "pdf"] = rel(root, path)
        hands = HANDS_RE.search(stem)
        if hands:
            entry["boards"] = int(hands.group(1))
    return sets


def merge_views(root, parent_dir):
    """Collect every view folder under a set-size (or All) folder, keyed by set number."""
    by_set = {}
    for view in VIEWS:
        vdir = os.path.join(parent_dir, view)
        if not os.path.isdir(vdir):
            continue
        for num, entry in scan_view_dir(root, vdir, view).items():
            slot = by_set.setdefault(num, {"set": num, "boards": None, "views": {}})
            boards = entry.pop("boards", None)
            entry.pop("set", None)
            if boards and not slot["boards"]:
                slot["boards"] = boards
            slot["views"][view] = entry
    return by_set


def find_intro(root, lesson_dir):
    for name in sorted(os.listdir(lesson_dir)):
        if name.endswith("_Intro.pdf"):
            return rel(root, os.path.join(lesson_dir, name))
    return None


def set_source_stem(lesson_dir):
    """The stem of the lesson PBN the sets were cut from, from the first set file found."""
    for _cur, _dirs, files in sorted(os.walk(lesson_dir)):
        for name in sorted(files):
            if SET_FILE_STEM_RE.search(name):
                return SET_FILE_STEM_RE.sub("", name)
    return None


def lesson_stem(lesson_dir):
    """The filename stem shared by the lesson's files, e.g. 'Baker Bridge Ogust'."""
    for name in sorted(os.listdir(lesson_dir)):
        if name.endswith("_Intro.pdf"):
            return name[: -len("_Intro.pdf")]
    stem = set_source_stem(lesson_dir)
    if stem and os.path.isfile(os.path.join(lesson_dir, stem + ".pbn")):
        return stem
    for name in sorted(os.listdir(lesson_dir)):
        if name.endswith(".pbn"):
            return os.path.splitext(name)[0]
    return os.path.basename(lesson_dir)


def is_flat(path):
    """A single-set lesson: its view folders sit directly in the lesson folder."""
    return any(os.path.isdir(os.path.join(path, v)) for v in VIEWS)


def is_lesson_dir(path):
    """A lesson folder holds an 'All N boards' or '{S}-Board Sets' child, or (a
    single-set lesson) the view folders themselves."""
    try:
        return is_flat(path) or any(
            ALL_DIR_RE.match(e) or SET_DIR_RE.match(e)
            for e in os.listdir(path) if os.path.isdir(os.path.join(path, e)))
    except OSError:
        return False


def scan_lesson(root, lesson_dir):
    entries = sorted(os.listdir(lesson_dir))
    stem = lesson_stem(lesson_dir)

    # The lesson PBN is the one its sets were cut from; the lesson folder may also hold
    # companion PBNs (e.g. exercises).
    lesson_pbn = None
    source = set_source_stem(lesson_dir)
    if source and os.path.isfile(os.path.join(lesson_dir, source + ".pbn")):
        lesson_pbn = rel(root, os.path.join(lesson_dir, source + ".pbn"))
    for name in entries:
        if lesson_pbn:
            break
        if name.endswith(".pbn") and os.path.isfile(os.path.join(lesson_dir, name)):
            lesson_pbn = rel(root, os.path.join(lesson_dir, name))

    out = {
        "name": os.path.basename(lesson_dir),
        "title": stem,
        "path": rel(root, lesson_dir),
        "boards": None,
        "intro": find_intro(root, lesson_dir),
        "lessonPbn": lesson_pbn,
        "all": None,
        "setSizes": [],
    }

    # Lesson-root PDFs besides the intro (lesson plans, rendered exercises). The key is
    # left out when there are none, so trees without them describe exactly as before.
    documents = [rel(root, os.path.join(lesson_dir, n)) for n in entries
                 if n.lower().endswith(".pdf") and not n.endswith("_Intro.pdf")
                 and os.path.isfile(os.path.join(lesson_dir, n))]
    if documents:
        out["documents"] = documents

    if is_flat(lesson_dir):
        by_set = merge_views(root, lesson_dir)
        whole = by_set.get(0) or next(iter(by_set.values()), None)
        if whole:
            out["boards"] = whole["boards"]
            out["all"] = {"boards": whole["boards"], "views": whole["views"]}

    for name in entries:
        sub = os.path.join(lesson_dir, name)
        if not os.path.isdir(sub):
            continue

        m = ALL_DIR_RE.match(name)
        if m:
            by_set = merge_views(root, sub)
            whole = by_set.get(0) or next(iter(by_set.values()), None)
            out["boards"] = int(m.group(1))
            if whole:
                out["all"] = {"boards": out["boards"], "views": whole["views"]}
            continue

        m = SET_DIR_RE.match(name)
        if m:
            by_set = merge_views(root, sub)
            sets = [by_set[k] for k in sorted(by_set)]
            out["setSizes"].append({"size": int(m.group(1)), "sets": sets})

    if out["boards"] is None and lesson_pbn:
        out["boards"] = count_boards(os.path.join(root, lesson_pbn))
    out["setSizes"].sort(key=lambda s: s["size"])
    return out


def build(root, name, generated_at):
    root = os.path.abspath(root)
    categories = []
    for cat in sorted(os.listdir(root)):
        cat_dir = os.path.join(root, cat)
        if not os.path.isdir(cat_dir):
            continue
        lessons = []
        for cur, subdirs, _files in os.walk(cat_dir):
            if is_lesson_dir(cur):
                lessons.append(scan_lesson(root, cur))
                subdirs[:] = []      # a lesson is a leaf; don't descend
        if lessons:
            lessons.sort(key=lambda l: l["name"])
            categories.append({"name": cat, "lessons": lessons})
    # A digest of what the manifest describes. Deterministic (the tree is walked in
    # sorted order), so an unchanged tree hashes the same on every build -- which is
    # what lets it serve as a cache-busting key without breaking reproducibility.
    content_hash = hashlib.sha256(
        json.dumps(categories, sort_keys=True, separators=(",", ":"),
                   ensure_ascii=False).encode("utf-8")).hexdigest()[:16]
    return {
        "schemaVersion": 1,
        "kind": "lesson-materials",
        "name": name,
        "generatedAt": generated_at or datetime.datetime.now().isoformat(),
        "contentHash": content_hash,
        "views": VIEWS,
        "categories": categories,
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("root", help="packaged materials tree (e.g. Rotations/)")
    ap.add_argument("--name", default="", help="collection display name")
    ap.add_argument("--generated-at", default=None, help="ISO-8601 timestamp (caller-supplied)")
    ap.add_argument("--out", default=None, help="output path (default: <root>/manifest.json)")
    a = ap.parse_args()

    if not os.path.isdir(a.root):
        print(f"Tree not found: {a.root}", file=sys.stderr)
        return 1

    manifest = build(a.root, a.name, a.generated_at)
    if not manifest["categories"]:
        print(f"No lessons found under {a.root}", file=sys.stderr)
        return 1

    out = a.out or os.path.join(a.root, "manifest.json")
    with open(out, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2, ensure_ascii=False)
        f.write("\n")

    lessons = sum(len(c["lessons"]) for c in manifest["categories"])
    sets = sum(len(s["sets"]) for c in manifest["categories"]
               for l in c["lessons"] for s in l["setSizes"])
    intros = sum(1 for c in manifest["categories"] for l in c["lessons"] if l["intro"])
    print(f"{len(manifest['categories'])} categories | {lessons} lessons | "
          f"{sets} board sets | {intros} intros")
    print(f"wrote {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
