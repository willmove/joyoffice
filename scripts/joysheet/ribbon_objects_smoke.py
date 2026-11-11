"""Structured Ribbon objects, page editing and auditing through production GPUI handlers."""
import base64
from PIL import Image


def exercise(send, wait, click, tab, shot, execute, checks, output):
    def fields(values):
        for key, value in values.items():
            send("ui.dialog.set", {"field": key, "value": value})

    def confirm():
        send("ui.dialog.confirm")
        send("ui.awaitIdle")
        assert not send("ui.inspect")["error"]

    def objects():
        return send("ui.inspect")["drawingObjects"]["shapes"]

    def bounds(object_id):
        return next(b for b in send("ui.inspect")["objectBounds"] if b["kind"] == "shape" and b["id"] == object_id)

    send("app.new")
    wait(lambda v: not v["drawingObjects"]["shapes"], "new objects workbook")
    source = output / "Example.txt"
    original = b"Embedded bytes: JoyOffice\r\n\x00\x01\xff"
    source.write_bytes(original)
    cases = [
        ("insert-smartart", "SmartArt", {"layout": "cycle", "text": "Plan\nExecute\nVerify", "at": "A3"}),
        ("word-art", "WordArt", {"text": "JoyOffice", "warp": "textArchUp", "at": "A14"}),
        ("insert-equation", "Equation", {"text": r"x=\frac{-b+\sqrt{b^{2}-4ac}}{2a}", "at": "F14"}),
        ("insert-object", "Embedded", {"path": str(source), "at": "J4"}),
    ]
    created = {}
    for button, kind, values in cases:
        tab("Insert")
        click("ribbon:" + button)
        wait(lambda v: v["dialog"] == "commands", kind + " dialog")
        fields(values)
        shot("dialog-" + kind.lower())
        confirm()
        obj = next(o for o in objects() if kind in (o["object"] or {}))
        created[kind] = obj["id"]
        assert obj["text"] == values.get("text", source.name)
        checks.append("Ribbon inserts editable " + kind + " through its form")

    b = bounds(created["WordArt"])
    send("ui.click", {"x": b["x"] + 30, "y": b["y"] + 30, "count": 2})
    wait(lambda v: v["dialog"] == "commands", "WordArt double-click editor")
    fields({"text": "Edited WordArt"})
    confirm()
    assert next(o for o in objects() if o["id"] == created["WordArt"])["text"] == "Edited WordArt"
    send("ui.key", {"key": "z", "ctrl": True})
    send("ui.awaitIdle")
    assert next(o for o in objects() if o["id"] == created["WordArt"])["text"] == "JoyOffice"
    send("ui.key", {"key": "y", "ctrl": True})
    send("ui.awaitIdle")
    checks.append("Double-click object editor and keyboard Undo/Redo update WordArt")

    b = bounds(created["Embedded"])
    send("ui.click", {"x": b["x"] + 30, "y": b["y"] + 30, "count": 2})
    wait(lambda v: v["dialog"] == "commands", "embedded extraction dialog")
    extracted = output / "extracted.txt"
    fields({"path": str(extracted)})
    confirm()
    assert extracted.read_bytes() == original
    checks.append("Embedded file extraction preserves every original byte")
    shot("objects-on-sheet")
    workbook = output / "objects-roundtrip.xlsx"
    send("app.save", {"path": str(workbook)})
    send("app.open", {"name": "objects-reopened.xlsx", "base64": base64.b64encode(workbook.read_bytes()).decode("ascii")})
    send("ui.awaitIdle")
    assert len(objects()) == 4
    assert any(o["text"] == "Edited WordArt" for o in objects())
    checks.append("Structured objects reopen independently with editable content intact")

    send("app.new")
    execute("range.setValues", {"range": "A1", "values": [["Date", "Region", "Sales"], ["1/1/2026", "East", 10], ["2/1/2026", "East", 20], ["3/1/2026", "West", 30]]})
    execute("insert.pivotTable", {"source": "Sheet1!A1:C4", "destination": "Sheet1!F3", "name": "SalesPivot", "rows": ["Region"], "values": ["Sales"]})
    tab("Insert")
    click("ribbon:insert-timeline")
    fields({"pivot": "SalesPivot", "column": "Date", "at": "A12"})
    confirm()
    timeline = next(o for o in objects() if "Timeline" in (o["object"] or {}))
    b = bounds(timeline["id"])
    tile_y = b["y"] + b["height"] - 40
    send("ui.click", {"x": b["x"] + 30, "y": tile_y})
    send("ui.awaitIdle")
    assert send("cell.get", {"cell": "G4"})["value"] == 10
    shot("timeline-filtered")
    scale = send("ui.inspect")["display"]["scaleFactor"]
    with Image.open(output / "timeline-filtered.png") as capture:
        divider = capture.convert("RGB").crop(tuple(round(n * scale) for n in
            (b["x"] + 24, b["y"] + 26, b["x"] + b["width"] - 24, b["y"] + 30)))
        assert sum(max(c) - min(c) > 20 and c[0] < 220 for c in divider.get_flattened_data()) >= divider.width / 2
    checks.append("Timeline header divider remains visible at fractional physical pixel width")
    send("ui.drag", {"x": b["x"] + 30, "y": tile_y, "toX": b["x"] + b["width"] / 2, "toY": tile_y})
    send("ui.awaitIdle")
    assert send("cell.get", {"cell": "G4"})["value"] == 30
    send("ui.click", {"x": b["x"] + b["width"] - 16, "y": b["y"] + 16})
    send("ui.awaitIdle")
    assert next(o for o in objects() if "Timeline" in (o["object"] or {}))["object"]["Timeline"]["selection"] is None
    send("ui.click", {"x": b["x"] + b["width"] - 48, "y": b["y"] + 42})
    wait(lambda v: v["dialog"] == "commands", "timeline level control")
    fields({"level": "days"})
    confirm()
    send("ui.click", {"x": b["x"] + b["width"] - 18, "y": b["y"] + b["height"] - 12})
    send("ui.awaitIdle")
    assert next(o for o in objects() if "Timeline" in (o["object"] or {}))["object"]["Timeline"]["scroll_start"] is not None
    checks.append("Timeline tiles, drag range, clear filter, level and scroll operate on PivotTable data")

    send("app.new")
    execute("range.setValues", {"range": "A1", "values": [[str(i), i] for i in range(1, 61)]})
    tab("View")
    click("ribbon:view-page-layout")
    view = wait(lambda v: v["pageView"]["paper"] is not None, "paper geometry")
    p, g = view["pageView"]["paper"], view["grid"]
    send("ui.click", {"x": g["x"] + p["left"] + p["pageWidth"] / 2, "y": g["y"] + p["top"] + p["marginTop"] / 2})
    wait(lambda v: v["dialog"] == "commands", "header section hit")
    fields({"text": "Header &[Page]"})
    confirm()
    assert "Header" in send("ui.inspect")["pageView"]["header"]
    send("ui.cell.click", {"cell": "B2"})
    send("ui.text", {"text": "Page edit"})
    send("ui.key", {"key": "enter"})
    send("ui.awaitIdle")
    assert send("cell.get", {"cell": "B2"})["value"] == "Page edit"
    shot("page-layout-edit")
    view = send("ui.inspect")
    row = next(r for r in view["visibleRows"] if r["index"] == 4)
    col = next(c for c in view["visibleColumns"] if c["index"] == 3)
    g, scale = view["grid"], view["display"]["scaleFactor"]
    with Image.open(output / "page-layout-edit.png") as capture:
        pixels = capture.convert("RGB")
        def grid_pixels(x, y, horizontal):
            x, y = round(x * scale), round(y * scale)
            return sum(all(abs(a - b) <= 3 for a, b in zip(pixels.getpixel((x + (i if horizontal else j), y + (j if horizontal else i))), (227, 233, 228)))
                       for i in range(-12, 13) for j in range(-2, 3))
        assert grid_pixels(g["x"] + col["start"] + col["size"] / 2,
                           g["y"] + row["start"] + row["size"], True) >= 15
        assert grid_pixels(g["x"] + col["start"] + col["size"],
                           g["y"] + row["start"] + row["size"] / 2, False) >= 15
    checks.append("Page Layout gridlines stay visible at fractional page origins")
    checks.append("Page Layout uses paper hit geometry for header and B2 editing")
    click("ribbon:view-page-break")
    send("ui.awaitIdle")
    execute("pageLayout.breaks", {"cell": "A12", "insert": True})
    view = send("ui.inspect")
    start = next(r["start"] for r in view["visibleRows"] if r["index"] == 11)
    end = next(r["start"] for r in view["visibleRows"] if r["index"] == 17)
    g = view["grid"]
    send("ui.drag", {"x": g["x"] + 150, "y": g["y"] + start, "toX": g["x"] + 150, "toY": g["y"] + end})
    send("ui.awaitIdle")
    assert send("ui.inspect")["pageView"]["rowBreaks"] == [17]
    shot("page-break-dragged")
    view = send("ui.inspect")
    g, scale = view["grid"], view["display"]["scaleFactor"]
    with Image.open(output / "page-break-dragged.png") as capture:
        outside = capture.convert("RGB").crop(tuple(round(n * scale) for n in (g["x"] + 300, g["y"] + 40, g["x"] + 1000, g["y"] + 400)))
        assert not any(b > 170 and r < 80 and green < 130 for r, green, b in outside.get_flattened_data())
    checks.append("Page Break Preview confines page numbers and break rules to the print area")
    checks.append("Page Break Preview drag moves a real saved print boundary")

    send("app.new")
    execute("range.setValues", {"range": "A1", "values": [[2, "=A1+1", "=B1*2"]]})
    execute("selection.set", {"range": "C1"})
    tab("Formulas")
    click("ribbon:trace-precedents")
    wait(lambda v: len(v["auditArrows"]) == 1, "first trace level")
    click("ribbon:trace-precedents")
    wait(lambda v: len(v["auditArrows"]) == 2, "second trace level")
    assert send("ui.inspect")["dataResultKind"] not in ("formulas.tracePrecedents", "formulas.traceDependents")
    shot("formula-trace-arrows")
    view = send("ui.inspect")
    row = next(r for r in view["visibleRows"] if r["index"] == 0)
    col = next(c for c in view["visibleColumns"] if c["index"] == 0)
    g, scale = view["grid"], view["display"]["scaleFactor"]
    with Image.open(output / "formula-trace-arrows.png") as capture:
        y = g["y"] + row["start"] + row["size"] / 2
        x = g["x"] + col["start"] + col["size"] / 2 + 12
        shaft = capture.convert("RGB").crop(tuple(round(n * scale) for n in (x, y - 2, x + 30, y + 2)))
        assert sum(b > 150 and r < 100 and green < 150 for r, green, b in shaft.get_flattened_data()) >= 15
        dot_x = g["x"] + col["start"] + col["size"] / 4
        dot = capture.convert("RGB").crop(tuple(round(n * scale) for n in
            (dot_x - 3, y - 3, dot_x + 3, y + 3)))
        assert sum(b > 150 and r < 100 and green < 100 for r, green, b in dot.get_flattened_data()) >= 8
    checks.append("Formula trace arrows paint a continuous horizontal shaft and blue origin dot")
    send("ui.command", {"command": "view.zoom", "params": {"percent": 150}})
    send("ui.awaitIdle")
    assert len(send("ui.inspect")["auditArrows"]) == 2
    click("ribbon:trace-remove")
    send("ui.awaitIdle")
    assert not send("ui.inspect")["auditArrows"]
    checks.append("Formula arrows expand levels, survive zoom refresh and remove through Ribbon")
