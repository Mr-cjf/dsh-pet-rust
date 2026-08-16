#!/usr/bin/env python3
"""Merge _action-frames animation frames into the main sprites JSON files.

- Reads src/assets/_action-frames/*.json (SD 40x29 action frames).
- Upscales each frame 2x: SD cell (x,y) -> HD 2x2 block
  (2x,2y),(2x+1,2y),(2x,2y+1),(2x+1,2y+1), producing a 58x80 HD frame.
- Merges the 5 action frames as {frames, frameDuration, loop} into
  whale-sprites-hd.json (HD frames) and whale-sprites.json (SD frames).
- Idempotent: rerunning produces identical output; the original 6 state
  fields are preserved untouched.
"""

import json
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ASSETS = os.path.join(ROOT, "src", "assets")
ACTION_DIR = os.path.join(ASSETS, "_action-frames")

ACTION_KEYS = ["move_swim", "turn_swing", "click_happy", "click_shy", "click_tsundere"]


def load_json(path):
    with open(path, "r", encoding="utf-8", newline="") as f:
        return json.load(f)


def save_json(path, data):
    # Keep the same file format as the original: indent=2 + CRLF + trailing newline.
    text = json.dumps(data, indent=2, ensure_ascii=False)
    text = text.replace("\n", "\r\n") + "\r\n"
    with open(path, "w", encoding="utf-8", newline="") as f:
        f.write(text)


def upscale2x(frame):
    """40x29 SD frame -> 80x58 HD frame: each char becomes a 2x2 block."""
    out = []
    for row in frame:
        doubled = "".join(ch + ch for ch in row)
        out.append(doubled)
        out.append(doubled)
    return out


def main():
    hd_path = os.path.join(ASSETS, "whale-sprites-hd.json")
    sd_path = os.path.join(ASSETS, "whale-sprites.json")
    hd = load_json(hd_path)
    sd = load_json(sd_path)

    for key in ACTION_KEYS:
        src = load_json(os.path.join(ACTION_DIR, key + ".json"))
        hd["sprites"][key] = {
            "frames": [upscale2x(f) for f in src["frames"]],
            "frameDuration": src["frameDuration"],
            "loop": src["loop"],
        }
        sd["sprites"][key] = {
            "frames": src["frames"],
            "frameDuration": src["frameDuration"],
            "loop": src["loop"],
        }

    save_json(hd_path, hd)
    save_json(sd_path, sd)
    print("merged %d action frames into whale-sprites-hd.json and whale-sprites.json" % len(ACTION_KEYS))


if __name__ == "__main__":
    main()
