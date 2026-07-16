//! HTML rendering for `report --html`. Self-contained (inline CSS), theme-aware,
//! deterministic (no timestamps), with difficulty cells heat-coloured on the
//! shared 0–5 scale.

use crate::profile::{CollectionProfile, TopicStats};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Render one or more collection profiles as a standalone HTML document.
pub fn render(profiles: &[CollectionProfile]) -> String {
    let mut h = String::new();
    h.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    h.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    h.push_str("<title>deal-survey — collection difficulty report</title>\n");
    h.push_str(STYLE);
    h.push_str("</head>\n<body>\n<main>\n");
    h.push_str("<h1>Lesson-collection difficulty report</h1>\n");
    h.push_str("<p class=\"note\">Cardplay ladder 0 cash-out · 1 establish · 2 single technique · 3 timing/entries · 4 advanced (squeeze/endplay). \
                Bidding 0 natural · 1 fundamental · 2 gadget · 3 upper-intermediate · 4 advanced · 5 expert. \
                <em>comb</em> columns = topic baseline nudged by observed difficulty.</p>\n");

    summary_table(&mut h, profiles);
    for p in profiles {
        collection_section(&mut h, p);
    }
    h.push_str("</main>\n</body>\n</html>\n");
    h
}

fn summary_table(h: &mut String, profiles: &[CollectionProfile]) {
    h.push_str("<h2>Collections</h2>\n");
    h.push_str("<p class=\"note\">The two coloured columns are the overall combined difficulty (mean over the collection), green → red.</p>\n");
    h.push_str("<table class=\"grid\">\n<thead><tr>");
    for c in ["collection", "deals", "cardplay", "bidding", "makeable", "L0", "L1", "L2", "uncl", "auction", "commentary", "expl. contract", "finesse", "ruff"] {
        let _ = write!(h, "<th>{}</th>", esc(c));
    }
    h.push_str("</tr></thead>\n<tbody>\n");
    for p in profiles {
        let mk = makeable(p);
        let d = &p.difficulty;
        // Overall combined difficulty = mean across the collection's lessons.
        let (cp_sum, cp_n) = p.by_lesson.values().fold((0, 0), |(s, n), t| {
            (s + t.combined_cardplay_sum, n + t.combined_cardplay_n)
        });
        let (bid_sum, bid_n) = p.by_lesson.values().fold((0, 0), |(s, n), t| {
            (s + t.combined_bidding_sum, n + t.combined_bidding_n)
        });
        let _ = write!(h, "<tr><td class=\"name\">{}</td>", esc(&p.collection));
        cell_int(h, p.deal_count);
        cell_comb(h, cp_sum, cp_n);
        cell_comb(h, bid_sum, bid_n);
        cell_pct(h, mk, p.deal_count);
        cell_pct(h, d.cash_out_0, mk);
        cell_pct(h, d.establish_1, mk);
        cell_pct(h, d.technique_2, mk);
        cell_pct(h, d.unclassified, mk);
        cell_pct(h, p.structural.with_auction, p.deal_count);
        cell_pct(h, p.structural.with_commentary, p.deal_count);
        cell_pct(h, p.structural.with_explicit_contract, p.deal_count);
        cell_int(h, *p.techniques.get("finesse").unwrap_or(&0));
        cell_int(h, *p.techniques.get("ruff").unwrap_or(&0));
        h.push_str("</tr>\n");
    }
    h.push_str("</tbody>\n</table>\n");
}

fn collection_section(h: &mut String, p: &CollectionProfile) {
    let _ = write!(h, "<h2>{} <span class=\"sub\">({} deals)</span></h2>\n", esc(&p.collection), p.deal_count);

    // Coverage / auction proxies / mix.
    let d = &p.difficulty;
    let a = &p.auction;
    let _ = write!(
        h,
        "<p class=\"stats\">excluded from ladder: not-makeable {}, no-contract {}, incomplete {}. ",
        d.not_makeable, d.no_contract, d.incomplete
    );
    if a.with_auction > 0 {
        let _ = write!(
            h,
            "auction: {:.1} bids avg, {} contested, {} doubles ({} final-dbl), {} high-bid deals.",
            a.total_bids as f64 / a.with_auction as f64,
            pct_str(a.contested, a.with_auction),
            a.total_doubles,
            a.double_of_final,
            a.with_high_bids
        );
    }
    h.push_str("</p>\n");
    dist_line(h, "Strain", &p.contract_mix.by_strain);
    dist_line(h, "Level", &p.contract_mix.by_level);
    dist_line(h, "Declarer", &p.contract_mix.by_declarer);

    if let Some(ed) = &p.editorial {
        h.push_str("<p class=\"editorial\">");
        for (k, v) in [
            ("Licensing", &ed.licensing),
            ("Audience", &ed.intended_audience),
            ("Commentary", &ed.commentary_quality),
            ("Notes", &ed.notes),
        ] {
            if let Some(v) = v {
                let _ = write!(h, "<strong>{}:</strong> {} ", esc(k), esc(v));
            }
        }
        h.push_str("</p>\n");
    }

    if let Some(topics) = &p.by_topic {
        h.push_str("<h3>By topic</h3>\n");
        breakdown_table(h, topics, true);
    }
    h.push_str("<h3>By lesson <span class=\"sub\">(grouped by category, hardest first)</span></h3>\n");
    lessons_by_category(h, p);
}

/// Per-lesson table grouped into category sections (each with a rollup row),
/// categories and lessons both ordered hardest-cardplay-first.
fn lessons_by_category(h: &mut String, p: &CollectionProfile) {
    if p.by_lesson.is_empty() {
        return;
    }
    h.push_str("<table class=\"grid\">\n<thead><tr>");
    for c in ["lesson", "deals", "base b", "base cp", "L0", "L1", "L2", "n-mk", "comb-cp", "comb-bid", "topic"] {
        let _ = write!(h, "<th>{}</th>", esc(c));
    }
    h.push_str("</tr></thead>\n<tbody>\n");

    // Categories ordered hardest first (by the category rollup's cardplay).
    let mut cats: Vec<(&String, &TopicStats)> = p.by_category.iter().collect();
    cats.sort_by(|a, b| ckey(b.1).partial_cmp(&ckey(a.1)).unwrap());

    for (cat, roll) in cats {
        // Category rollup row.
        let _ = write!(
            h,
            "<tr class=\"cat\"><td class=\"catname\" colspan=\"8\">{} <span class=\"sub\">({} deals)</span></td>",
            esc(cat), roll.deal_count
        );
        cell_comb(h, roll.combined_cardplay_sum, roll.combined_cardplay_n);
        cell_comb(h, roll.combined_bidding_sum, roll.combined_bidding_n);
        h.push_str("<td></td></tr>\n");

        // Lessons in this category, hardest first.
        let mut rows: Vec<(&String, &TopicStats)> = p
            .by_lesson
            .iter()
            .filter(|(_, t)| &t.category == cat)
            .collect();
        rows.sort_by(|a, b| ckey(b.1).partial_cmp(&ckey(a.1)).unwrap().then(b.1.deal_count.cmp(&a.1.deal_count)));
        for (lesson, t) in rows {
            let _ = write!(h, "<tr><td class=\"name indent\">{}</td>", esc(&lesson_label(lesson)));
            cell_int(h, t.deal_count);
            cell_base(h, t.baseline_bidding);
            cell_base(h, t.baseline_cardplay);
            cell_int(h, t.observed_cardplay[0]);
            cell_int(h, t.observed_cardplay[1]);
            cell_int(h, t.observed_cardplay[2]);
            cell_int(h, t.not_makeable);
            cell_comb(h, t.combined_cardplay_sum, t.combined_cardplay_n);
            cell_comb(h, t.combined_bidding_sum, t.combined_bidding_n);
            let _ = write!(h, "<td class=\"topic\">{}</td>", esc(&t.topic));
            h.push_str("</tr>\n");
        }
    }
    h.push_str("</tbody>\n</table>\n");
}

/// Sort key: combined cardplay mean (–1 when none, so empty sinks).
fn ckey(t: &TopicStats) -> f64 {
    cmean(t.combined_cardplay_sum, t.combined_cardplay_n)
}

/// Shared table for by-topic / by-lesson (rows sorted hardest cardplay first).
fn breakdown_table(h: &mut String, map: &BTreeMap<String, TopicStats>, is_topic: bool) {
    let mut rows: Vec<(&String, &TopicStats)> = map.iter().collect();
    rows.sort_by(|x, y| {
        cmean(y.1.combined_cardplay_sum, y.1.combined_cardplay_n)
            .partial_cmp(&cmean(x.1.combined_cardplay_sum, x.1.combined_cardplay_n))
            .unwrap()
            .then(
                cmean(y.1.combined_bidding_sum, y.1.combined_bidding_n)
                    .partial_cmp(&cmean(x.1.combined_bidding_sum, x.1.combined_bidding_n))
                    .unwrap(),
            )
            .then(y.1.deal_count.cmp(&x.1.deal_count))
    });

    h.push_str("<table class=\"grid\">\n<thead><tr>");
    let first = if is_topic { "topic" } else { "lesson" };
    let mut heads = vec![first, "deals", "base b", "base cp", "L0", "L1", "L2", "n-mk", "comb-cp", "comb-bid"];
    if !is_topic {
        heads.push("topic");
    }
    for c in heads {
        let _ = write!(h, "<th>{}</th>", esc(c));
    }
    h.push_str("</tr></thead>\n<tbody>\n");
    for (key, t) in rows {
        let label = if is_topic { key.clone() } else { lesson_label(key) };
        let _ = write!(h, "<tr><td class=\"name\">{}</td>", esc(&label));
        cell_int(h, t.deal_count);
        cell_base(h, t.baseline_bidding);
        cell_base(h, t.baseline_cardplay);
        cell_int(h, t.observed_cardplay[0]);
        cell_int(h, t.observed_cardplay[1]);
        cell_int(h, t.observed_cardplay[2]);
        cell_int(h, t.not_makeable);
        cell_comb(h, t.combined_cardplay_sum, t.combined_cardplay_n);
        cell_comb(h, t.combined_bidding_sum, t.combined_bidding_n);
        if !is_topic {
            let _ = write!(h, "<td class=\"topic\">{}</td>", esc(&t.topic));
        }
        h.push_str("</tr>\n");
    }
    h.push_str("</tbody>\n</table>\n");
}

// --- cells ------------------------------------------------------------------

fn cell_int(h: &mut String, n: usize) {
    let _ = write!(h, "<td class=\"num\">{n}</td>");
}
fn cell_pct(h: &mut String, n: usize, d: usize) {
    let _ = write!(h, "<td class=\"num\">{}</td>", pct_str(n, d));
}
fn cell_base(h: &mut String, v: u8) {
    let _ = write!(h, "<td class=\"num heat\" style=\"{}\">{v}</td>", heat(v as f64));
}
/// A combined-score cell, heat-coloured on the 0–5 scale.
fn cell_comb(h: &mut String, sum: usize, n: usize) {
    if n == 0 {
        h.push_str("<td class=\"num\">–</td>");
    } else {
        let m = sum as f64 / n as f64;
        let _ = write!(h, "<td class=\"num heat\" style=\"{}\">{m:.1}</td>", heat(m));
    }
}

/// Green (easy) → red (hard) background for a 0–5 difficulty value.
fn heat(v: f64) -> String {
    let t = (v / 5.0).clamp(0.0, 1.0);
    let hue = 140.0 * (1.0 - t); // 140 green → 0 red
    format!("background:hsl({hue:.0} 62% 42%);color:#fff")
}

fn dist_line(h: &mut String, label: &str, map: &BTreeMap<String, usize>) {
    if map.is_empty() {
        return;
    }
    let parts: Vec<String> = map.iter().map(|(k, v)| format!("{}:{}", esc(k), v)).collect();
    let _ = write!(h, "<p class=\"dist\"><strong>{}</strong> {}</p>\n", esc(label), parts.join("  "));
}

fn makeable(p: &CollectionProfile) -> usize {
    let d = &p.difficulty;
    d.cash_out_0 + d.establish_1 + d.technique_2 + d.unclassified
}
fn pct_str(n: usize, d: usize) -> String {
    if d == 0 {
        "–".to_string()
    } else {
        format!("{:.0}%", 100.0 * n as f64 / d as f64)
    }
}
fn cmean(sum: usize, n: usize) -> f64 {
    if n > 0 {
        sum as f64 / n as f64
    } else {
        -1.0
    }
}
fn lesson_label(path: &str) -> String {
    let base = path.rsplit('/').next().unwrap_or(path);
    base.strip_suffix(".pbn").unwrap_or(base).to_string()
}
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

const STYLE: &str = r#"<style>
:root{--bg:#fbfbfa;--fg:#1a1a1a;--muted:#666;--line:#e3e3e0;--head:#f0f0ee;--zebra:#f6f6f4;}
@media (prefers-color-scheme:dark){:root{--bg:#16181c;--fg:#e6e6e6;--muted:#9aa0a6;--line:#2c2f36;--head:#20242b;--zebra:#1b1e24;}}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--fg);font:14px/1.5 -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;}
main{max-width:1100px;margin:0 auto;padding:24px 20px 64px;}
h1{font-size:24px;margin:0 0 4px;}
h2{font-size:19px;margin:34px 0 8px;padding-top:10px;border-top:1px solid var(--line);}
h3{font-size:15px;margin:20px 0 6px;color:var(--muted);text-transform:uppercase;letter-spacing:.04em;}
.sub{color:var(--muted);font-weight:400;font-size:.8em;}
.note,.stats,.dist,.editorial{color:var(--muted);font-size:13px;margin:6px 0;}
.dist strong{color:var(--fg);} .editorial strong{color:var(--fg);}
.wrap{overflow-x:auto;}
table.grid{border-collapse:collapse;width:100%;margin:8px 0 4px;font-variant-numeric:tabular-nums;}
table.grid th,table.grid td{padding:5px 9px;border-bottom:1px solid var(--line);text-align:right;white-space:nowrap;}
table.grid th{position:sticky;top:0;background:var(--head);text-align:right;font-weight:600;font-size:12px;}
table.grid td.name,table.grid th:first-child{text-align:left;}
table.grid td.name{font-weight:500;}
table.grid td.topic{text-align:left;color:var(--muted);font-size:12px;}
table.grid tbody tr:nth-child(even){background:var(--zebra);}
td.num{text-align:right;}
td.heat{font-weight:600;border-radius:3px;}
table.grid tr.cat td{background:var(--head);font-weight:700;border-top:2px solid var(--line);}
table.grid td.catname{text-align:left;letter-spacing:.02em;}
table.grid td.name.indent{padding-left:22px;font-weight:400;}
</style>
"#;
