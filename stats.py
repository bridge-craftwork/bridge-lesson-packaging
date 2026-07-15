#!/usr/bin/env python3
"""Lesson-collection statistics — emit a stats JSON + an HTML report.

Collection-agnostic: point it at a packaged collection tree (the standard
`{category}/{lesson}/...` structure from CONTRACT.md) and it reports the shape of the
collection. It reads only the packaged output; it knows nothing about any specific collection.

v1 metrics:
  - lesson count
  - boards (hands) per lesson: min / max / median / mean + distribution
  - whether each lesson has a Declarer's Plan artifact
  - whether each lesson's PBN carries a [SkillPath]
  - build durations (passed in via --durations, included verbatim)

Extensible: per-lesson records carry a `difficulty` slot for later auction/play/defense levels.

Usage:
    python3 stats.py <collection_root> [--name NAME] [--durations durations.json]
        [--json stats.json] [--html stats.html] [--generated-at ISO8601]
"""
import argparse, glob, json, os, re, statistics, sys

SET_OR_VIEW = re.compile(r'(Set \d| \d+x\d+|- \d+x\d+|NESW\.pbn$|-NS\.pbn$|-S\.pbn$'
                         r'| NS\.pbn$| S\.pbn$)')
BOARD_RE = re.compile(r'^\[Board ', re.M)
SKILLPATH_RE = re.compile(r'\[SkillPath\s+"([^"]*)"\]')


def _is_lesson_dir(d):
    """A lesson folder either (a) has a `Full Table` child (rotated-view layout) or
    (b) has a full-lesson PBN directly at its root (lesson-root layout). Handles collections
    that nest lessons at varying depth."""
    entries = os.listdir(d)
    if any(e.lower() == "full table" and os.path.isdir(os.path.join(d, e)) for e in entries):
        return True
    return any(e.endswith(".pbn") and not SET_OR_VIEW.search(e) for e in entries)


def lesson_dirs(root):
    """Walk the tree; a lesson is the shallowest dir matching `_is_lesson_dir` (don't descend
    into it). `category` = the top-level folder under root. Depth-robust."""
    out = []
    root = os.path.abspath(root)
    for cur, subdirs, _files in os.walk(root):
        if cur == root:
            continue
        try:
            if _is_lesson_dir(cur):
                rel = os.path.relpath(cur, root)
                category = rel.split(os.sep)[0]
                out.append((category, os.path.basename(cur), cur))
                subdirs[:] = []  # prune: don't recurse into a lesson
        except OSError:
            continue
    out.sort(key=lambda t: (t[0], t[1]))
    return out


def full_lesson_pbn(lesson_dir):
    """The full-lesson PBN = the shallowest PBN that is not a sliced set, block-replication,
    or a single-view file. Returns path or None."""
    cands = []
    for p in glob.glob(os.path.join(lesson_dir, "**", "*.pbn"), recursive=True):
        if SET_OR_VIEW.search(os.path.basename(p)):
            continue
        depth = os.path.relpath(p, lesson_dir).count(os.sep)
        cands.append((depth, len(p), p))
    if not cands:
        # fall back to any PBN (e.g. only sets exist)
        alls = glob.glob(os.path.join(lesson_dir, "**", "*.pbn"), recursive=True)
        return min(alls, key=len) if alls else None
    cands.sort()
    return cands[0][2]


def board_count(pbn):
    try:
        return len(BOARD_RE.findall(open(pbn, encoding="utf-8", errors="replace").read()))
    except OSError:
        return 0


def skillpath_of(pbn):
    try:
        m = SKILLPATH_RE.search(open(pbn, encoding="utf-8", errors="replace").read())
        return m.group(1) if m else None
    except OSError:
        return None


def has_declarer_plan(lesson_dir):
    return bool(glob.glob(os.path.join(lesson_dir, "**", "*Declarer*Plan*.pdf"), recursive=True)
                or glob.glob(os.path.join(lesson_dir, "**", "*declarer*plan*.pdf"), recursive=True))


def distribution(values):
    b = {"1-6": 0, "7-12": 0, "13-20": 0, "21+": 0}
    for v in values:
        b["1-6" if v <= 6 else "7-12" if v <= 12 else "13-20" if v <= 20 else "21+"] += 1
    return b


def build_stats(root, name, durations, generated_at):
    lessons = []
    for cat, lesson, ldir in lesson_dirs(root):
        pbn = full_lesson_pbn(ldir)
        lessons.append({
            "category": cat,
            "lesson": lesson,
            "boards": board_count(pbn) if pbn else 0,
            "declarerPlan": has_declarer_plan(ldir),
            "skillPath": skillpath_of(pbn) if pbn else None,
            "difficulty": None,   # reserved: auction / play / defense levels (future)
        })
    boards = [l["boards"] for l in lessons if l["boards"]]
    stats = {
        "collection": name,
        "generatedAt": generated_at,
        "lessonCount": len(lessons),
        "boards": {
            "total": sum(boards),
            "min": min(boards) if boards else 0,
            "max": max(boards) if boards else 0,
            "median": int(statistics.median(boards)) if boards else 0,
            "mean": round(statistics.mean(boards), 1) if boards else 0,
            "distribution": distribution(boards),
        },
        "declarerPlan": {
            "withPlan": sum(1 for l in lessons if l["declarerPlan"]),
            "withoutPlan": sum(1 for l in lessons if not l["declarerPlan"]),
        },
        "skillPath": {
            "withSkillPath": sum(1 for l in lessons if l["skillPath"]),
            "withoutSkillPath": sum(1 for l in lessons if not l["skillPath"]),
        },
        "buildDurations": durations,   # verbatim from --durations, or null
        "lessons": lessons,
    }
    return stats


def html_report(s):
    def row(l):
        return (f"<tr><td>{esc(l['category'])}</td><td>{esc(l['lesson'])}</td>"
                f"<td class=n>{l['boards']}</td>"
                f"<td class=c>{'✓' if l['declarerPlan'] else ''}</td>"
                f"<td class=c>{'✓' if l['skillPath'] else ''}</td>"
                f"<td class=sp>{esc(l['skillPath'] or '')}</td></tr>")
    esc = lambda x: (str(x).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;"))
    b = s["boards"]
    dur = ""
    if s.get("buildDurations"):
        items = "".join(f"<tr><td>{esc(k)}</td><td class=n>{esc(v)}</td></tr>"
                        for k, v in s["buildDurations"].items())
        dur = f"<h2>Build durations</h2><table class=kv>{items}</table>"
    dist = "".join(f"<tr><td>{k}</td><td class=n>{v}</td></tr>"
                   for k, v in b["distribution"].items())
    return f"""<!doctype html><meta charset=utf-8>
<title>{esc(s['collection'] or 'Lesson collection')} — statistics</title>
<style>
 body{{font:14px/1.5 system-ui,sans-serif;margin:2rem;color:#1a1a1a}}
 h1{{margin:0 0 .25rem}} .sub{{color:#666;margin:0 0 1.5rem}}
 table{{border-collapse:collapse;margin:.5rem 0 1.5rem}} th,td{{padding:.3rem .6rem;border-bottom:1px solid #eee;text-align:left}}
 th{{border-bottom:2px solid #ccc}} .n{{text-align:right;font-variant-numeric:tabular-nums}} .c{{text-align:center}}
 .sp{{color:#666;font:12px ui-monospace,monospace}} .cards{{display:flex;gap:1rem;flex-wrap:wrap;margin:1rem 0}}
 .card{{border:1px solid #ddd;border-radius:8px;padding:.75rem 1rem;min-width:8rem}}
 .card .v{{font-size:1.6rem;font-weight:600}} .card .k{{color:#666;font-size:.85rem}}
 table.kv td:first-child{{color:#444}}
</style>
<h1>{esc(s['collection'] or 'Lesson collection')}</h1>
<p class=sub>statistics{(' — ' + esc(s['generatedAt'])) if s.get('generatedAt') else ''}</p>
<div class=cards>
 <div class=card><div class=v>{s['lessonCount']}</div><div class=k>lessons</div></div>
 <div class=card><div class=v>{b['total']}</div><div class=k>total boards</div></div>
 <div class=card><div class=v>{b['median']}</div><div class=k>median boards/lesson</div></div>
 <div class=card><div class=v>{b['min']}–{b['max']}</div><div class=k>boards range</div></div>
 <div class=card><div class=v>{s['declarerPlan']['withPlan']}</div><div class=k>with declarer's plan</div></div>
 <div class=card><div class=v>{s['skillPath']['withSkillPath']}/{s['lessonCount']}</div><div class=k>with skill path</div></div>
</div>
<h2>Boards per lesson</h2><table><tr><th>range</th><th>lessons</th></tr>{dist}</table>
{dur}
<h2>Lessons</h2>
<table><tr><th>category</th><th>lesson</th><th>boards</th><th>decl. plan</th><th>skill path</th><th>path</th></tr>
{''.join(row(l) for l in s['lessons'])}
</table>"""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("root")
    ap.add_argument("--name", default="")
    ap.add_argument("--durations", help="JSON file of build durations (included verbatim)")
    ap.add_argument("--json", dest="json_out", default="stats.json")
    ap.add_argument("--html", dest="html_out", default="stats.html")
    ap.add_argument("--generated-at", default=None, help="ISO-8601 timestamp (caller-supplied)")
    a = ap.parse_args()
    if not os.path.isdir(a.root):
        print(f"not a directory: {a.root}", file=sys.stderr); sys.exit(2)
    durations = json.load(open(a.durations)) if a.durations and os.path.exists(a.durations) else None
    s = build_stats(a.root, a.name, durations, a.generated_at)
    json.dump(s, open(a.json_out, "w"), indent=2)
    open(a.html_out, "w").write(html_report(s))
    print(f"{s['lessonCount']} lessons | boards {s['boards']['min']}–{s['boards']['max']} "
          f"(median {s['boards']['median']}) | declarer's plan {s['declarerPlan']['withPlan']} "
          f"| skillPath {s['skillPath']['withSkillPath']}/{s['lessonCount']}")
    print(f"wrote {a.json_out}, {a.html_out}")


if __name__ == "__main__":
    main()
