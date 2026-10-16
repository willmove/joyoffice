#!/usr/bin/env python3
"""Document-length scaling (BENCH_PAGES) and cold-start runs."""
import json
import os
import subprocess
import time

B = os.path.dirname(os.path.abspath(__file__))


def run(v, args, env, timeout=180):
    cmd = ["/usr/bin/time", "-l", os.path.join(B, "target-rel", v, "release", "bench")] + args
    e = dict(os.environ)
    e.update(env)
    t0 = time.time()
    p = subprocess.run(cmd, env=e, capture_output=True, text=True, timeout=timeout)
    elapsed = time.time() - t0
    rec = None
    for line in p.stdout.splitlines():
        if line.startswith("RESULT "):
            rec = json.loads(line[7:])
    rss = None
    for line in p.stderr.splitlines():
        if "maximum resident set size" in line:
            rss = int(line.split()[0])
    if rec is None:
        print("FAILED", v, args, env, p.stderr[-600:], flush=True)
        return None
    rec.update({"bin": v, "flags": args, "env": env, "process_wall_s": round(elapsed, 3),
                "max_rss_mb": round(rss / 1048576, 1) if rss else None})
    return rec


with open(os.path.join(B, "results_scaling.jsonl"), "a") as out:
    for rep in range(3):
        for pages in ("8", "64"):
            for v, flags in (("v022", []), ("vpre", []), ("vpre", ["--cached"]), ("vfast", []),
                             ("vkit", []), ("vkitfast", [])):
                for s in ("blink", "type", "scroll"):
                    r = run(v, ["--scenario", s, "--frames", "300", "--warmup", "60"] + flags,
                            {"BENCH_PAGES": pages})
                    if r:
                        r["rep"] = rep
                        out.write(json.dumps(r, ensure_ascii=False) + "\n")
                        out.flush()
                        print(rep, pages, v, flags, s, "%.3f" % r["draw_cpu_mean_ms"], flush=True)

with open(os.path.join(B, "results_startup.jsonl"), "a") as out:
    for rep in range(8):
        for v in ("v022", "vpre", "vfast", "vkit", "vkitfast"):
            r = run(v, ["--scenario", "blink", "--frames", "1", "--warmup", "0"], {})
            if r:
                r["rep"] = rep
                out.write(json.dumps(r, ensure_ascii=False) + "\n")
                out.flush()
                print("startup", rep, v, r["process_wall_s"], r["draw_cpu_max_ms"], r["max_rss_mb"],
                      flush=True)
