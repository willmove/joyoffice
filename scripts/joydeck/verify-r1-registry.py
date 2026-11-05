"""Reconcile the historical DeckCraft inventory with the built JoyDeck CLI."""

import argparse
import csv
import hashlib
import json
from pathlib import Path
from queue import Empty, Queue
import shutil
import subprocess
import tempfile
from threading import Thread

ROOT = Path(__file__).resolve().parents[2]
INVENTORY = ROOT / "docs/joydeck/COMMANDS.csv"
FIXTURE = ROOT / "tests/fixtures/joydeck/independent.pptx"
PROTOCOL = "joydeck-control-1"


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def inventory():
    with INVENTORY.open(encoding="utf-8-sig", newline="") as stream:
        rows = list(csv.DictReader(stream))
    ids = {row["command_id"] for row in rows}
    assert len(rows) == len(ids) == 329
    return ids


def probe_control(cli, root, source, upstream):
    # The live open response supplies the document ID for every validation.
    with subprocess.Popen(
        [cli, "control", "--root", root],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    ) as process:
        inbox = Queue()

        def read():
            for line in process.stdout:
                inbox.put(line)
            inbox.put(None)

        Thread(target=read, daemon=True).start()
        serial = 0

        def call(method, params=None, state=None):
            nonlocal serial
            serial += 1
            request = {
                "protocolVersion": PROTOCOL,
                "requestId": str(serial),
                "method": method,
                "params": params or {},
            }
            if state is not None:
                request["target"] = state["documentId"]
                request["expectedRevision"] = state["revision"]
            process.stdin.write(json.dumps(request) + "\n")
            process.stdin.flush()
            try:
                line = inbox.get(timeout=15)
            except Empty as error:
                raise RuntimeError(f"control timed out on {method}") from error
            assert line is not None, f"control stopped on {method}"
            reply = json.loads(line)
            assert reply["requestId"] == request["requestId"], reply
            return reply

        try:
            catalog = call("engine.commands")["result"]
            opened = call("document.open", {"path": str(source)})["result"]
            baseline = call("document.inspect", state=opened)["result"]
            assert opened["pages"] == len(baseline["slides"]) == 3
            assert not opened["dirty"] and not opened["canUndo"]
            validated = call("command.validate", {"command": {
                "command": "insert-rect", "x": 5, "y": 5, "width": 20, "height": 15,
            }}, opened)["result"]
            assert validated["valid"] and validated["wouldChange"] and not validated["sideEffects"]

            unregistered_edits = upstream.difference(catalog["commands"], catalog["legacyCommandSubset"])
            for command_id in sorted(unregistered_edits):
                legacy = call("command.validate", {"command": command_id}, opened)["error"]
                assert legacy["code"] == "operation-failed" and "outside the advertised subset" in legacy["message"], (command_id, legacy)
                typed = call("command.validate", {"command": {"command": command_id}}, opened)["error"]
                assert typed["code"] == "operation-failed" and "unknown variant" in typed["message"], (command_id, typed)

            assert call("document.status", state=opened)["result"] == opened
            assert call("document.inspect", state=opened)["result"] == baseline
        finally:
            process.stdin.close()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
                raise
        assert process.returncode == 0
    return catalog, len(unregistered_edits)


def mcp_catalog(cli, root):
    request = {
        "protocolVersion": PROTOCOL,
        "requestId": "registry",
        "method": "engine.commands",
        "params": {},
    }
    messages = [
        {
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "joydeck-r1-registry", "version": "1"},
            },
        },
        {"jsonrpc": "2.0", "method": "notifications/initialized"},
        {
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {"name": "joydeck.control", "arguments": request},
        },
    ]
    output = subprocess.run(
        [cli, "mcp", "--root", root],
        input="".join(json.dumps(message) + "\n" for message in messages),
        capture_output=True, encoding="utf-8", timeout=15, check=True,
    ).stdout
    ready, tools = [json.loads(line) for line in output.splitlines()]
    assert ready["id"] == 1 and ready["result"]["protocolVersion"] == "2025-11-25"
    assert tools["id"] == 2
    reply = tools["result"]["structuredContent"]
    assert reply["requestId"] == request["requestId"]
    return reply["result"]


def verify(cli):
    upstream = inventory()
    cli_commands = json.loads(subprocess.check_output([cli, "commands"], encoding="utf-8", timeout=15))
    with tempfile.TemporaryDirectory(prefix="joydeck-r1-registry-") as temporary:
        root = Path(temporary).resolve()
        source = root / "语义 对照.pptx"
        shutil.copyfile(FIXTURE, source)
        control, rejected_ids = probe_control(cli, root, source, upstream)
        mcp = mcp_catalog(cli, root)
        assert digest(source) == digest(FIXTURE)

    fields = ("commands", "methods", "legacyCommandSubset")
    for field in fields:
        values = control[field]
        assert values and len(values) == len(set(values)), field
        assert values == mcp[field], field
    for catalog in (control, mcp):
        assert catalog["protocolVersion"] == PROTOCOL
        assert catalog["commandSchemaVersion"] == 2
        assert catalog["legacyCommandSchemaSupported"] is False
    assert cli_commands == control["commands"]
    overlaps = {field: sorted(upstream.intersection(control[field])) for field in fields}
    return {
        "inventorySha256": digest(INVENTORY),
        "fixtureSha256": digest(FIXTURE),
        "cliSha256": digest(cli),
        "upstreamIdCount": len(upstream),
        "typedSchema": control["commandSchemaVersion"],
        "typedCommands": cli_commands,
        "controlMethods": control["methods"],
        "legacyEditSubset": control["legacyCommandSubset"],
        "exactUpstreamIdMatches": overlaps,
        "upstreamIdsRefusedInBothEditSchemas": rejected_ids,
        "typedEditValidationIsReadOnly": True,
        "negativeProbesPreservedSessionAndSource": True,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--evidence", type=Path)
    args = parser.parse_args()
    report = verify(args.cli.resolve(strict=True))
    if args.evidence:
        args.evidence.parent.mkdir(parents=True, exist_ok=True)
        args.evidence.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(
        f"R1 registry reconciled {report['upstreamIdCount']} upstream IDs with "
        f"{len(report['typedCommands'])} typed edits, {len(report['controlMethods'])} methods, "
        f"and {len(report['legacyEditSubset'])} legacy edits."
    )


if __name__ == "__main__":
    main()
