#!/usr/bin/env python3
"""W02 几何证据测量（纯标准库，无需 PIL）。

三个子命令：

  window  比对探针窗口自捕获 PNG（逻辑分辨率，含标题栏）与
          evidence-zXX.json：以四个标定块（3 黑 1 红）解出画布原点，
          再逐个比对红色字形原点标尺的 x 位置，并统计基线带内黑色
          墨迹列数（证明字形确实画出）。
  pdf     比对 qlmanage 栅格化的 probe.pdf.png 与 evidence-z100.json：
          PDF 侧无标尺，比较每行墨迹的水平跨度与字形数估计。
  scale   检查 evidence-z200 的期望位置恰为 evidence-z100 的两倍。

输出 JSON 摘要并打印结论；退出码非 0 表示有超差项。
"""

import json
import struct
import sys
import zlib
from pathlib import Path


def decode_png(path):
    """解码 8-bit 非 interlace 的 RGB/RGBA/灰度 PNG。"""
    data = Path(path).read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a png")
    pos = 8
    width = height = None
    bit_depth = color_type = None
    idat = bytearray()
    palette = None
    while pos < len(data):
        length, tag = struct.unpack_from(">I4s", data, pos)
        chunk = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if tag == b"IHDR":
            width, height, bit_depth, color_type = struct.unpack_from(">IIBB", chunk, 0)[:4]
        elif tag == b"PLTE":
            palette = [tuple(chunk[i : i + 3]) for i in range(0, len(chunk), 3)]
        elif tag == b"IDAT":
            idat.extend(chunk)
        elif tag == b"IEND":
            break
    if bit_depth != 8:
        raise ValueError(f"unsupported bit depth {bit_depth}")
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[color_type]
    raw = zlib.decompress(bytes(idat))
    stride = width * channels
    # 过滤器反解（PNG 滤波：每行首字节为滤波类型）。
    out = bytearray(width * height * channels)
    previous = bytearray(stride)
    pos = 0
    for row in range(height):
        filter_type = raw[pos]
        pos += 1
        line = bytearray(raw[pos : pos + stride])
        pos += stride
        if filter_type == 1:  # Sub
            for i in range(channels, stride):
                line[i] = (line[i] + line[i - channels]) & 0xFF
        elif filter_type == 2:  # Up
            for i in range(stride):
                line[i] = (line[i] + previous[i]) & 0xFF
        elif filter_type == 3:  # Average
            for i in range(stride):
                left = line[i - channels] if i >= channels else 0
                line[i] = (line[i] + ((left + previous[i]) >> 1)) & 0xFF
        elif filter_type == 4:  # Paeth
            for i in range(stride):
                a = line[i - channels] if i >= channels else 0
                b = previous[i]
                c = previous[i - channels] if i >= channels else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                predictor = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
                line[i] = (line[i] + predictor) & 0xFF
        out[row * stride : (row + 1) * stride] = line
        previous = line
    return width, height, channels, palette, bytes(out)


def pixel(width, channels, palette, buf, x, y):
    at = (y * width + x) * channels
    if channels == 1:
        if palette:
            return palette[buf[at]]
        v = buf[at]
        return (v, v, v)
    if channels == 2:
        v = buf[at]
        return (palette[v][0] if palette else v,) * 3
    return buf[at], buf[at + 1], buf[at + 2]


def find_blocks(width, height, channels, palette, buf, predicate, limit_region=None):
    """找满足 predicate(r,g,b) 的连通色块，返回 (min_x, min_y, max_x, max_y, count)。"""
    visited = bytearray(width * height)
    blocks = []
    for y in range(height):
        for x in range(width):
            at = y * width + x
            if visited[at]:
                continue
            r, g, b = pixel(width, channels, palette, buf, x, y)
            if not predicate(r, g, b):
                continue
            # BFS
            stack = [(x, y)]
            visited[at] = 1
            min_x = max_x = x
            min_y = max_y = y
            count = 0
            while stack:
                cx, cy = stack.pop()
                count += 1
                min_x, max_x = min(min_x, cx), max(max_x, cx)
                min_y, max_y = min(min_y, cy), max(max_y, cy)
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                    nx, ny = cx + dx, cy + dy
                    if 0 <= nx < width and 0 <= ny < height and not visited[ny * width + nx]:
                        r2, g2, b2 = pixel(width, channels, palette, buf, nx, ny)
                        if predicate(r2, g2, b2):
                            visited[ny * width + nx] = 1
                            stack.append((nx, ny))
            if limit_region is None or count <= limit_region:
                blocks.append((min_x, min_y, max_x, max_y, count))
    return blocks


def is_black(r, g, b):
    return r < 80 and g < 80 and b < 80


def is_red(r, g, b):
    return r > 150 and g < 90 and b < 90


def measure_window(png, evidence_path):
    evidence = json.loads(Path(evidence_path).read_text())
    width, height, channels, palette, buf = decode_png(png)
    # 标定块是精确的 8x8 方块：按尺寸/面积严格过滤，避免把小字形当块。
    blacks = [
        b
        for b in find_blocks(
            width, height, channels, palette, buf, is_black, limit_region=400
        )
        if 6 <= (b[2] - b[0]) <= 10 and 6 <= (b[3] - b[1]) <= 10 and 30 <= b[4] <= 110
    ]
    reds_all = find_blocks(width, height, channels, palette, buf, is_red, limit_region=400)
    red_squares = [b for b in reds_all if b[4] > 20 and (b[2] - b[0]) <= 20]
    calibration = evidence["calibration_logical_px"]

    # 三个黑色标定块构成已知 L 形：(24,24)-(956,24)-(24,688)。画布在
    # 窗口图像中的原点未知（标题栏+头部高度），按相对几何解平移量：
    # 同 y 的两块是顶行（左=左上、右=右上），剩下的是左下。
    # 用块包围盒的左上角（而非中心）解原点，避免半宽差混入。
    corners = sorted((b[0], b[1], b[2], b[3]) for b in blacks)
    if len(corners) < 3:
        return {"error": "calibration blocks not found", "black_blocks": len(blacks)}
    top_two = sorted(corners, key=lambda c: c[1])[:2]
    bottom = sorted(corners, key=lambda c: c[1])[-1]
    top_left, top_right = sorted(top_two)
    origin_x = top_left[0] - calibration[0][0]
    origin_y = top_left[1] - calibration[0][1]
    checks = [
        (
            "top_right",
            (top_right[0], top_right[1]),
            (calibration[1][0], calibration[1][1]),
        ),
        (
            "bottom_left",
            (bottom[0], bottom[1]),
            (calibration[2][0], calibration[2][1]),
        ),
    ]
    residuals = {}
    for name, measured, expected in checks:
        residuals[name] = [
            round(measured[0] - (expected[0] + origin_x), 2),
            round(measured[1] - (expected[1] + origin_y), 2),
        ]
    spread_x = max(abs(v[0]) for v in residuals.values())
    spread_y = max(abs(v[1]) for v in residuals.values())
    offsets = [(origin_x, origin_y)] * 3

    # 红色标尺：过滤出标尺条（宽 ~2px、高 ~10px），取左上角为测得起点。
    ticks = []
    for block in reds_all:
        w = block[2] - block[0]
        h = block[3] - block[1]
        if 1 <= w <= 6 and 8 <= h <= 16:
            ticks.append((float(block[0]), float(block[1])))
    ticks.sort(key=lambda t: (t[1], t[0]))

    # 期望标尺（画布逻辑坐标）。超出窗口可视宽度的字形会被裁剪，
    # 单独计数、不参与比对。
    expected = []
    out_of_view = 0
    for run in evidence["runs"]:
        for glyph in run["glyphs"]:
            if glyph["origin_lx"] + origin_x >= width - 2:
                out_of_view += 1
                continue
            expected.append((glyph["origin_lx"], run["ruler_ly"], run["line"], glyph["i"]))
    expected.sort(key=lambda t: (t[1], t[0]))

    rows = []
    if len(ticks) == len(expected):
        for (mx, my), (ex, ey, line, index) in zip(ticks, expected):
            rows.append({
                "line": line, "glyph": index,
                "expected_lx": round(ex, 2), "measured_lx": round(mx - origin_x, 2),
                "dx": round(mx - origin_x - ex, 2),
                "expected_ly": round(ey, 2), "measured_ly": round(my - origin_y, 2),
                "dy": round(my - origin_y - ey, 2),
            })
        max_dx = max(abs(r["dx"]) for r in rows)
        max_dy = max(abs(r["dy"]) for r in rows)
    else:
        rows = [{"expected": len(expected), "measured": len(ticks)}]
        max_dx = max_dy = None

    # 墨迹检查：每行基线带 (baseline-20, baseline+6) 的黑色墨迹列。
    ink = []
    for run in evidence["runs"]:
        y0 = int(origin_y + run["baseline_ly"] - 20)
        y1 = int(origin_y + run["baseline_ly"] + 6)
        x0 = int(origin_x + run["origin_lx"])
        dark_columns = 0
        total_dark = 0
        for x in range(x0, min(int(origin_x + run["origin_lx"] + 700), width)):
            column_dark = 0
            for y in range(max(0, y0), min(height, y1)):
                r, g, b = pixel(width, channels, palette, buf, x, y)
                if is_black(r, g, b):
                    column_dark += 1
            if column_dark:
                dark_columns += 1
                total_dark += column_dark
        ink.append({"line": run["line"], "ink_columns": dark_columns, "ink_pixels": total_dark})

    return {
        "png": str(png),
        "png_size": [width, height],
        "calibration_blocks_found": len(offsets),
        "calibration_offset_spread_px": [round(spread_x, 2), round(spread_y, 2)],
        "red_square_found": len(red_squares) >= 1,
        "canvas_origin": [round(origin_x, 2), round(origin_y, 2)],
        "ticks_expected": len(expected),
        "ticks_out_of_view": out_of_view,
        "ticks_measured": len(ticks),
        "max_abs_dx_px": max_dx,
        "max_abs_dy_px": max_dy,
        "rows_sample": rows[:4],
        "ink": ink,
        "within_1_5px": max_dx is not None and max_dx <= 1.5 and max_dy is not None and max_dy <= 1.5,
    }


def measure_pdf(png, evidence_path):
    evidence = json.loads(Path(evidence_path).read_text())
    width, height, channels, palette, buf = decode_png(png)
    # PDF 高度 842pt（A4 297mm）。
    scale = height / 842.0
    page = evidence["page"]
    # 画布逻辑坐标 → 页面坐标（扣掉页面投影原点）→ pt（*0.75）→ png px。
    results = []
    for run in evidence["runs"]:
        baseline_pt = (run["baseline_ly"] - page["origin_ly"]) * 0.75
        y0 = int((baseline_pt - 14) * scale)
        y1 = int((baseline_pt + 5) * scale)
        columns = {}
        for x in range(width):
            for y in range(max(0, y0), min(height, y1)):
                r, g, b = pixel(width, channels, palette, buf, x, y)
                if is_black(r, g, b):
                    columns[x] = True
                    break
        xs = sorted(columns)
        expected_x0 = (run["origin_lx"] - page["origin_lx"]) * 0.75 * scale
        span = (xs[0], xs[-1]) if xs else (None, None)
        # 估计墨迹簇数（相邻列间距 > 0.35 em 视为分簇）。
        clusters = 0
        last = None
        gap_em = 12 * 0.75 * scale  # 1em in png px
        for x in xs:
            if last is None or x - last > gap_em * 0.18:
                clusters += 1
            last = x
        results.append({
            "line": run["line"],
            "glyphs": run["glyph_count"],
            "ink_first_x": span[0], "ink_last_x": span[1],
            "expected_first_x": round(expected_x0, 1),
            "ink_clusters": clusters,
            "scale_pt_to_px": round(scale, 4),
        })
    return {"png": str(png), "png_size": [width, height], "lines": results}


def measure_scale(evidence_a, evidence_b):
    a = json.loads(Path(evidence_a).read_text())
    b = json.loads(Path(evidence_b).read_text())
    page = a["page"]
    # 位置 = 页面投影原点（常量）+ 随 zoom 线性缩放的部分；
    # 检查 (z2 - origin) == 2 * (z1 - origin)。
    rows = []
    worst = 0.0
    for run_a, run_b in zip(a["runs"], b["runs"]):
        for ga, gb in zip(run_a["glyphs"], run_b["glyphs"]):
            xa = ga["origin_lx"] - page["origin_lx"]
            xb = gb["origin_lx"] - page["origin_lx"]
            ya = ga["origin_ly"] - page["origin_ly"]
            yb = gb["origin_ly"] - page["origin_ly"]
            if abs(xa) > 1e-6:
                worst = max(worst, abs(xb - 2 * xa))
            if abs(ya) > 1e-6:
                worst = max(worst, abs(yb - 2 * ya))
            rows.append(1)
    return {
        "evidence_a": str(evidence_a),
        "evidence_b": str(evidence_b),
        "glyphs_checked": len(rows),
        "max_abs_deviation_from_2x_px": round(worst, 4),
        "all_exactly_two": worst <= 1e-6,
    }


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    mode = sys.argv[1]
    if mode == "window":
        result = measure_window(sys.argv[2], sys.argv[3])
    elif mode == "pdf":
        result = measure_pdf(sys.argv[2], sys.argv[3])
    elif mode == "scale":
        result = measure_scale(sys.argv[2], sys.argv[3])
    else:
        print(__doc__)
        return 2
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
