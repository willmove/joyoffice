"""Measure fresh-process font discovery and a second scan through the linked Word and Deck APIs."""
import argparse
import json
import os
from pathlib import Path
import platform
import subprocess
from tempfile import TemporaryDirectory

PROBE = """
extern crate catalog;
use std::time::Instant;
fn main() {
    let start = Instant::now();
    let db = catalog::FontDb::global();
    let count = db.load_system_fonts();
    let cold = start.elapsed().as_micros();
    let next = Instant::now();
    let again = db.load_system_fonts();
    assert_eq!(count, again);
    println!("{} {} {}", count, cold, next.elapsed().as_micros());
}
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--manifest-path", type=Path, default=Path("Cargo.toml"))
    parser.add_argument("--samples", type=int, default=5)
    args = parser.parse_args()
    root = args.manifest_path.resolve().parent
    env = dict(os.environ, CRAFT_I18N_DIR=str(root / "third-party/craft-i18n"))
    result = subprocess.run(["cargo","build","--locked","--release","--message-format=json",
                            "-p","joyword-fonts","-p","joydeck-fonts"],cwd=root,env=env,check=True,stdout=subprocess.PIPE,text=True)
    report = {"platform":platform.platform(),"build":"release","samplesPerProduct":args.samples,"products":{}}
    with TemporaryDirectory() as work:
        src = Path(work)/"probe.rs"
        src.write_text(PROBE)
        for line in result.stdout.splitlines():
            data = json.loads(line)
            name = data.get("target",{}).get("name")
            if name not in ("joyword_fonts","joydeck_fonts") or data["reason"] != "compiler-artifact":
                continue
            lib = next(Path(p) for p in data["filenames"] if p.endswith(".rlib"))
            deps = lib.parent/"deps" if lib.parent.name != "deps" else lib.parent
            executable = Path(work)/name
            subprocess.run(["rustc","--edition=2024","-O","-C","lto=thin",src,"--extern",f"catalog={lib}",
                            "-L",f"dependency={deps}","-o",executable],cwd=root,env=env,check=True)
            runs = []
            for _ in range(args.samples):
                count,cold,again = map(int,subprocess.check_output([executable],cwd=root,env=env,text=True).split())
                runs.append({"faces":count,"coldUs":cold,"rescanUs":again})
            report["products"][name] = runs
    assert len(report["products"]) == 2
    args.output.write_text(json.dumps(report,indent=2) + "\n")


if __name__=="__main__":
    main()
