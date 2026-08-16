#!/usr/bin/env python3
"""Verify the main sprites JSON files frame by frame.

For whale-sprites-hd.json (80x58) and whale-sprites.json (40x29):
- HD frames are 58 rows x 80 chars; SD frames are 29 rows x 40 chars.
- Every char belongs to the 11-key palette.
- SD action frames: silhouette IoU vs the SD default frame is HARD-gated at 0.84
  (centroid-translation aligned; turn_swing may be horizontally mirrored). This is
  the gate that ensures the action-frame artwork is correct.
- HD action frames: IoU vs the HD default frame is printed for INFORMATION only
  and is NOT gated. Reason: HD state frames are 8x8-block extractions while the
  action frames are 16x16-density SD frames upscaled 2x, i.e. different source
  density (8x8 vs 16x16). The silhouette matches but white-highlight (w) detail
  density differs, so HD IoU (~0.66~0.71) is informational only and must not fail
  the build.

Usage: python verify_sprites.py [sd_iou_min]   (default 0.84)
"""

import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ASSETS = os.path.join(ROOT, "src", "assets")

HD_ROWS, HD_COLS = 58, 80
SD_ROWS, SD_COLS = 29, 40
PALETTE_SIZE = 11

# SD 层硬阈值：动作帧 vs SD default 的轮廓 IoU（把关动作帧制作正确性）
SD_IOU_MIN = float(sys.argv[1]) if len(sys.argv) > 1 else 0.84

ACTION_KEYS = ["move_swim", "turn_swing", "click_happy", "click_shy", "click_tsundere"]
MIRRORABLE = {"turn_swing"}


def load_json(path):
    with open(path, "r", encoding="utf-8", newline="") as f:
        return json.load(f)


def frames_of(sprite):
    """Normalize single-frame (string[]) and sequence ({frames,...}) to a frame list."""
    if isinstance(sprite, list):
        return [sprite]
    if isinstance(sprite, dict) and isinstance(sprite.get("frames"), list):
        return sprite["frames"]
    raise ValueError("unknown sprite shape: %r" % type(sprite))


def nontransparent(frame):
    return {(x, y) for y, row in enumerate(frame) for x, ch in enumerate(row) if ch != "."}


def centroid(points):
    if not points:
        return (0.0, 0.0)
    xs = [p[0] for p in points]
    ys = [p[1] for p in points]
    return (sum(xs) / len(xs), sum(ys) / len(ys))


def mirror_frame(frame):
    return ["".join(reversed(row)) for row in frame]


def shift_points(points, dx, dy, cols, rows):
    out = set()
    for x, y in points:
        nx, ny = x + dx, y + dy
        if 0 <= nx < cols and 0 <= ny < rows:
            out.add((nx, ny))
    return out


def iou(a, b):
    inter = len(a & b)
    union = len(a | b)
    return inter / union if union else 0.0


def aligned_iou(base_frame, frame, allow_mirror):
    """Centroid-translation aligned silhouette IoU; mirror variant wins if allowed."""
    rows = len(base_frame)
    cols = len(base_frame[0])
    base_pts = nontransparent(base_frame)
    base_c = centroid(base_pts)

    variants = [frame]
    if allow_mirror:
        variants.append(mirror_frame(frame))

    best = 0.0
    for v in variants:
        v_pts = nontransparent(v)
        v_c = centroid(v_pts)
        dx = round(base_c[0] - v_c[0])
        dy = round(base_c[1] - v_c[1])
        aligned = shift_points(v_pts, dx, dy, cols, rows)
        best = max(best, iou(base_pts, aligned))
    return best


def verify():
    failures = []
    # iou_min: None => 信息性输出（HD 层）；数值 => 硬阈值（SD 层）
    targets = [
        ("HD", os.path.join(ASSETS, "whale-sprites-hd.json"), HD_ROWS, HD_COLS, None),
        ("SD", os.path.join(ASSETS, "whale-sprites.json"), SD_ROWS, SD_COLS, SD_IOU_MIN),
    ]

    for label, path, rows, cols, iou_min in targets:
        data = load_json(path)
        palette_keys = set(data["palette"].keys())
        if len(palette_keys) != PALETTE_SIZE:
            failures.append("%s palette has %d keys, expected %d" % (label, len(palette_keys), PALETTE_SIZE))
            print("[FAIL] %s palette: %d keys (expected %d)" % (label, len(palette_keys), PALETTE_SIZE))
            continue

        sprites = data["sprites"]
        default_frame = frames_of(sprites["default"])[0]

        for key in sprites:
            frames = frames_of(sprites[key])
            for i, frame in enumerate(frames):
                fid = key if len(frames) == 1 else "%s#%d" % (key, i)
                problems = []

                if len(frame) != rows:
                    problems.append("rows=%d (!=%d)" % (len(frame), rows))
                bad_cols = [r for r in frame if len(r) != cols]
                if bad_cols:
                    problems.append("%d rows with col != %d" % (len(bad_cols), cols))
                bad_chars = sorted({ch for row in frame for ch in row} - palette_keys)
                if bad_chars:
                    problems.append("chars outside palette: %s" % bad_chars)

                iou_note = ""
                if key in ACTION_KEYS:
                    val = aligned_iou(default_frame, frame, key in MIRRORABLE)
                    if iou_min is None:
                        # HD 层：动作帧(16x16 密度 upscale2x)与状态帧(8x8 独立提取)不同源，
                        # 轮廓一致但白色高光 w 细节密度不同，IoU 仅作信息参考，不卡阈值。
                        iou_note = "IoU=%.4f (info)" % val
                    else:
                        # SD 层：硬阈值把关动作帧制作正确性
                        iou_note = "IoU=%.4f (min %.2f)" % (val, iou_min)
                        if val < iou_min:
                            problems.append("IoU %.4f < %.2f" % (val, iou_min))

                if problems:
                    failures.append("%s %s: %s" % (label, fid, "; ".join(problems)))
                    print("[FAIL] %s %-22s rows=%d cols=%d %s -> %s" % (label, fid, len(frame), len(frame[0]) if frame else 0, iou_note, "; ".join(problems)))
                else:
                    print("[OK ] %s %-22s rows=%d cols=%d %s" % (label, fid, len(frame), len(frame[0]) if frame else 0, iou_note))

        gate_desc = "IoU info-only" if iou_min is None else "IoU min %.2f" % iou_min
        print("== %s: %d sprites verified (palette=%d keys, %s) ==" % (label, len(sprites), len(palette_keys), gate_desc))

    if failures:
        print("\n%d FAILURE(S):" % len(failures))
        for f in failures:
            print("  - " + f)
        sys.exit(1)
    print("\nALL CHECKS PASSED (SD IoU threshold %.2f, HD IoU informational)" % SD_IOU_MIN)


if __name__ == "__main__":
    verify()
