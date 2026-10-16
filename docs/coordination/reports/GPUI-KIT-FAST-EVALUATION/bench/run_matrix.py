#!/usr/bin/env python3
"""Run the frame-cost matrix; append one JSON object per run to results.jsonl.

Each run is a separate process with its own short-lived, non-activating
pop-up window. Variants are interleaved per repetition to spread drift.
"""
import json
import os
import subprocess
import sys
import time

B = os.path.dirname(os.path.abspath(__file__))
REPS = int(sys.argv[1]) if len(sys.argv) > 1 else 3
FRAMES = sys.argv[2] if len(sys.argv) > 2 else "600"
OUT = os.path.join(B, sys.argv[3] if len(sys.argv) > 3 else "results.jsonl")


def binp(v):
    return os.path.join(B, "target-rel", v, "release", "bench")


SCEN = ["blink", "type", "scroll", "refresh"]
CASES = []
for s in SCEN:
    CASES.append(("v022", s, [], {}))
    CASES.append(("v022", s, ["--cached"], {}))
    CASES.append(("vpre", s, [], {}))
    CASES.append(("vpre", s, ["--cached"], {}))
    CASES.append(("vfast", s, [], {}))
    CASES.append(("vfast", s, [], {"GPUI_VIEW_RETENTION": "0"}))
    CASES.append(("vkit", s, [], {}))
    CASES.append(("vkitfast", s, [], {}))
# JoyWord-like editing surface: IME handler registered inside scrolled content.
CASES.append(("vpre", "scroll", ["--ime"], {}))
CASES.append(("vfast", "scroll", ["--ime"], {}))
CASES.append(("vfast", "scroll", [], {"GPUI_SCROLL_LAYERS": "0"}))
CASES.append(("vfast", "blink", ["--ime"], {}))
CASES.append(("vfast", "type", ["--ime"], {}))

only = os.environ.get("ONLY")
if only:
    CASES = [c for c in CASES if c[0] in only.split(",")]

with open(OUT, "a") as out:
    for rep in range(REPS):
        for v, s, flags, env in CASES:
            cmd = ["/usr/bin/time", "-l", binp(v), "--scenario", s, "--frames", FRAMES, "--warmup", "120"] + flags
            e = dict(os.environ)
            e.update(env)
            t0 = time.time()
            try:
                p = subprocess.run(cmd, env=e, capture_output=True, text=True, timeout=180)
            except subprocess.TimeoutExpired:
                print("TIMEOUT", v, s, flags, env, flush=True)
                continue
            rec = None
            for line in p.stdout.splitlines():
                if line.startswith("RESULT "):
                    rec = json.loads(line[7:])
            rss = None
            for line in p.stderr.splitlines():
                if "maximum resident set size" in line:
                    rss = int(line.split()[0])
            if rec is None:
                print("FAILED", v, s, flags, env, p.returncode, p.stderr[-800:], flush=True)
                continue
            rec.update({"bin": v, "flags": flags, "env": env, "rep": rep,
                        "max_rss_mb": round(rss / 1048576, 1) if rss else None,
                        "elapsed_s": round(time.time() - t0, 2)})
            out.write(json.dumps(rec, ensure_ascii=False) + "\n")
            out.flush()
            print(rep, v, s, flags, env, "cpu_mean=%.3f p95=%.3f glyphs=%.0f rss=%s" % (
                rec["draw_cpu_mean_ms"], rec["draw_cpu_p95_ms"], rec["glyphs_painted_per_frame"],
                rec["max_rss_mb"]), flush=True)
            time.sleep(0.2)
