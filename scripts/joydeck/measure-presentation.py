"""Measure an actual edit on a media-rich slide through the OS-presented canvas pixels."""
import argparse
import json
import platform
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from desktop_frames import display_stream
from native_pilot_support import ROOT, DeckControl, app, digest, wait
from probe_native_ime import run


def measure(binary, observer, out):
    source = ROOT / "tests/fixtures/joydeck/r6-r7/delivery-media.pptx"
    config = {"JOYDECK_CONFIG_DIR":str(out / "profile"), "JOYDECK_RECOVERY_DIR":str(out / "recovery")}
    args = [str(source), "--window-size=1100x740", "--control-stdio=" + str(out)]
    with app(binary, args, out, config) as process:
        client = DeckControl(process)
        before = wait(client.inspect, "media presentation", process)
        client.edit({"command":{"command":"content", "action":{"kind":"background", "rgb":0x13B7CA}}})
        assert client.request("document.status")["dirty"]
        scale = 1.
        mac = platform.system() == "Darwin"
        if not mac:
            xid = run("xdotool","search","--onlyvisible","--pid",str(process.pid)).splitlines()[0]
            geom = dict(s.split("=",1) for s in run("xdotool","getwindowgeometry","--shell",xid).splitlines())
            scale = int(geom["WIDTH"]) / 1100
        crop = [round(v * scale) for v in (395,305,230,125)]
        hashes, samples = {}, []
        with display_stream(observer, process.pid, crop, out) as (frames, match):
            time.sleep(.7)
            for i in range(24):
                method = ("document.undo", "document.redo")[i % 2]
                guard = client.guard()
                start = time.monotonic_ns()
                result = client.request(method, **guard)
                assert result["changed"] and result["engineRevision"] > 0
                if i < 2:
                    time.sleep(.35)
                    # The native renderer submits asynchronously. Read a fresh crop after it settles.
                    stamp = time.monotonic_ns()
                    client.request("document.status")
                    _, hashes[method] = match(stamp)
                    if i == 1 and len(set(hashes.values())) != 2:
                        raise ValueError(f"slide edit did not change the displayed crop {crop}")
                else:
                    try:
                        ns, _ = match(start, hashes[method], timeout=.25)
                    except TimeoutError:
                        # An idle window may need another exposure for the X observer to sample it.
                        client.request("document.status")
                        ns, _ = match(start, hashes[method])
                    if i >= 4:
                        samples.append(round((ns - start) / 1e6, 4))
        assert len(before["slides"]) == len(client.inspect()["slides"])
    return {"platform":platform.platform(), "binarySha256":digest(binary), "fixtureSha256":digest(source),
            "operation":"undo and redo a background-color edit on the first media-rich slide",
            "start":"immediately before sending the guarded mutation to the live desktop",
            "end":"WindowServer displayTime" if mac else "X Present pixmap CompleteNotify UST, pixels read on delivery",
            "crop":crop, "canvasHashes":hashes, "samplesMs":samples, "p95Ms":sorted(samples)[18]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("binary", "observer", "output"):
        parser.add_argument(arg, type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    data = measure(args.binary.resolve(), args.observer.resolve(), out)
    (out / "report.json").write_text(json.dumps(data, indent=2) + "\n")
