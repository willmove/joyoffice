#!/usr/bin/env python3
"""Aggregate benchmark JSONL: median over repetitions, min-max spread."""
import json
import os
import statistics
import sys
from collections import defaultdict

B = os.path.dirname(os.path.abspath(__file__))


def label(r):
    tag = r["bin"]
    if r.get("flags"):
        tag += " " + " ".join(r["flags"])
    for k, v in sorted(r.get("env", {}).items()):
        if k != "BENCH_PAGES":
            tag += f" {k}={v}"
    return tag


def load(name):
    p = os.path.join(B, name)
    if not os.path.exists(p):
        return []
    return [json.loads(l) for l in open(p) if l.strip()]


def table(rows, key_fn, metric):
    groups = defaultdict(list)
    for r in rows:
        groups[key_fn(r)].append(r[metric])
    return {k: (statistics.median(v), min(v), max(v), len(v)) for k, v in groups.items()}


rows = load("results.jsonl")
if rows:
    print("## frame matrix: draw_cpu_mean_ms (median of reps [min-max], n)")
    t = table(rows, lambda r: (label(r), r["scenario"]), "draw_cpu_mean_ms")
    t95 = table(rows, lambda r: (label(r), r["scenario"]), "draw_cpu_p95_ms")
    tg = table(rows, lambda r: (label(r), r["scenario"]), "glyphs_painted_per_frame")
    tr = table(rows, lambda r: (label(r), r["scenario"]), "max_rss_mb")
    labels = sorted({k[0] for k in t}, key=lambda s: (["v022", "vpre", "vfast", "vkit", "vkitfast"].index(s.split()[0]), s))
    scen = ["Blink", "Type", "Scroll", "Refresh"]
    print("| variant | " + " | ".join(scen) + " |")
    print("|---|" + "---|" * len(scen))
    for l in labels:
        cells = []
        for s in scen:
            if (l, s) in t:
                m, lo, hi, n = t[(l, s)]
                p95 = t95[(l, s)][0]
                g = tg[(l, s)][0]
                cells.append(f"{m:.3f} (p95 {p95:.3f}; {lo:.3f}-{hi:.3f}; n={n}; glyphs {g:.0f})")
            else:
                cells.append("-")
        print(f"| {l} | " + " | ".join(cells) + " |")
    print()
    print("max RSS MB (median):", {f"{k[0]}/{k[1]}": v[0] for k, v in tr.items() if k[1] == "Blink"})

rows = load("results_scaling.jsonl")
if rows:
    print("\n## scaling: draw_cpu_mean_ms median by pages")
    t = table(rows, lambda r: (label(r), r["scenario"], r["pages"]), "draw_cpu_mean_ms")
    for l in sorted({k[0] for k in t}):
        for s in ("Blink", "Type", "Scroll"):
            a = t.get((l, s, 8), (float("nan"),))[0]
            b = t.get((l, s, 64), (float("nan"),))[0]
            print(f"| {l} | {s} | {a:.3f} | {b:.3f} | x{b / a:.1f} |")

rows = load("results_startup.jsonl")
if rows:
    print("\n## startup (median of reps): process wall s, first draw cpu ms, max RSS MB")
    for v in ("v022", "vpre", "vfast", "vkit", "vkitfast"):
        rs = [r for r in rows if r["bin"] == v]
        if not rs:
            continue
        print(f"| {v} | {statistics.median(r['process_wall_s'] for r in rs):.3f} | "
              f"{statistics.median(r['draw_cpu_max_ms'] for r in rs):.2f} | "
              f"{statistics.median(r['max_rss_mb'] for r in rs):.1f} | n={len(rs)} |")
