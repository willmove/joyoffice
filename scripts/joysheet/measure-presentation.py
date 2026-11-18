"""Time a Sheet commit to the displayed frame containing the new dependent cell pixels."""
import argparse
import json
from pathlib import Path
import platform
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from desktop_frames import display_stream
from native_pilot_support import SheetControl, app, digest, port
from probe_native_ime import run


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ("binary","observer","output"):
        p.add_argument(name,type=Path)
    p.add_argument("--rows",type=int,choices=(100,100000),default=100000)
    args=p.parse_args()
    out=args.output.resolve()
    out.mkdir(parents=True,exist_ok=False)
    profile=out/"profile"
    number=port()
    client=SheetControl(number,profile)
    call=client.call
    with app(args.binary.resolve(),["--control",str(number),"--diagnostics"],out,
             {"JOYSHEET_CONFIG_DIR":str(profile)},unset=("JOYSHEET_CONTROL_TOKEN","GRIDCRAFT_CONTROL_TOKEN")) as proc:
        client.wait(lambda v:v["perf"]["frame"]>1,"worksheet",proc)
        for first in range(1,args.rows+1,5000):
            values=[[f"=$A$1+{i}"] for i in range(first,min(first+5000,args.rows+1))]
            call("engine.execute",command="range.setValues",params={"range":f"B{first}","values":values})
            call("ui.awaitIdle")
        scale=1.
        if platform.system()=="Linux":
            call("ui.resize", width=920, height=740)
            client.wait(lambda v: abs(v["grid"]["width"]-920)<100, "window resized",proc)
            time.sleep(.3)
            xid=run("xdotool","search","--onlyvisible","--pid",str(proc.pid)).splitlines()[0]
            physical=dict(line.split("=",1) for line in run("xdotool","getwindowgeometry","--shell",xid).splitlines())
            scale=int(physical["WIDTH"])/920.
        view=call("ui.inspect")
        column=next(item for item in view["visibleColumns"] if item["index"]==1)
        row=next(item for item in view["visibleRows"] if item["index"]==0)
        # Restrict observation to B1:B3. The active cell, caret and editor always stay in column A.
        crop=[round(view["grid"]["x"]+column["start"]+2),round(view["grid"]["y"]+row["start"]+1),
              round(column["size"])-4,round(row["size"]*3)-2]
        crop=[round(v*scale) for v in crop]
        schedule=[17,88,17,88]+[17,88]*10
        hashes={}
        samples=[]
        with display_stream(args.observer.resolve(),proc.pid,crop,out) as (frames,match):
            time.sleep(1)
            for i,v in enumerate(schedule):
                call("ui.cell.click",cell="A1")
                call("ui.edit.text",text=str(v))
                begin=time.monotonic_ns()
                call("ui.edit.commit")
                call("ui.awaitIdle")
                assert call("cell.get",cell="B1")["value"]==v+1
                if i<2:
                    match(begin)
                    time.sleep(.32)
                    hashes[v]=frames[-1][1]
                    if i==1 and len(set(hashes.values()))!=2:
                        raise ValueError(f"the two computed cell values have identical captured pixels at {crop}")
                else:
                    stamp,_=match(begin,hashes[v])
                    if i>=4:
                        samples.append(round((stamp-begin)/1e6,4))
    method="ScreenCaptureKit WindowServer displayTime" if platform.system()=="Darwin" else "X Present pixmap CompleteNotify UST, crop read at event delivery"
    report={"system":platform.platform(),"rows":args.rows,"binarySha256":digest(args.binary),"method":method,
            "origin":"immediately before sending the edit.commit request over the loopback control transport",
            "crop":crop,"alternatingCellPixelHashes":list(hashes.values()),"samplesMs":samples,"p95Ms":sorted(samples)[18]}
    (out/"report.json").write_text(json.dumps(report,indent=2)+"\n")


if __name__=="__main__":
    main()
