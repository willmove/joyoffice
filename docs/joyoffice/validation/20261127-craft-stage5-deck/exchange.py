"""Replay the stage 3 exchange suite with the known percentage-label schema change."""

import copy
import importlib.util
import json
from pathlib import Path
import sys

from audit import ROOT


spec = importlib.util.spec_from_file_location(
    "stage3_exchange", ROOT / "scripts/joydeck/check-stage3-package-exchange.py"
)
stage3 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stage3)
strict_equal = stage3.assert_equal
checked = []


def compare_with_percentage_labels(frozen, candidate, label):
    if frozen == candidate:
        return
    if not isinstance(candidate, dict) or not {"slides", "nextId"} <= candidate.keys():
        strict_equal(frozen, candidate, label)
        return
    projected = copy.deepcopy(candidate)
    charts = [
        (slide["id"], shape["id"], shape["kind"])
        for slide in projected["slides"] for shape in slide["shapes"]
        if shape["kind"]["type"] == "chart"
    ]
    # The two M9 Office fixtures share the same five charts and slide identifiers.
    expected = [
        (1000779, 3, "column", False),
        (1000780, 3, "bar", False),
        (1000781, 3, "lineMarkers", False),
        (1000782, 3, "pie", True),
        (1000783, 3, "scatter", False),
    ]
    observed = [
        (slide, shape, chart["kind"], chart.get("dataLabelPercent", False))
        for slide, shape, chart in charts
    ]
    if observed != expected:
        strict_equal(frozen, candidate, label)
        return
    for (_, _, chart), (_, _, kind, enable) in zip(charts, expected):
        assert len(chart["series"]) == 1, (label, kind)
        value = chart.pop("dataLabelPercent", False)
        assert value is enable, (label, kind, value)
        assert chart["series"][0].pop("dataLabelPercent", None) is enable, (label, kind)
    strict_equal(frozen, projected, label)
    checked.append(label)


if __name__ == "__main__":
    stage3.assert_equal = compare_with_percentage_labels
    stage3.main()
    assert len(checked) == 12, checked
    report = Path(sys.argv[1]) / "verification.json"
    data = json.loads(report.read_text(encoding="utf-8"))
    data["knownChartPercentLabelComparisons"] = checked
    report.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n",
                      encoding="utf-8", newline="\n")
