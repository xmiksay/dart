#!/usr/bin/env python3
"""Phase-1 parity gate: the Rust/candle detector must reproduce the Ultralytics
(Python) detector on the same images, within a pixel tolerance.

It runs Ultralytics in-process and shells out to the Rust binary
(`detect --json`), matches detections per class by nearest centre, and reports
the worst pixel error plus any unmatched detections. Exit code 0 = parity holds.

IMPORTANT: both detectors must use the same input size. candle's `IMG_SIZE` is
640 (src/detect/mod.rs) and confidence is 0.25 (CONF_THRESHOLD) — the defaults
here match. If you change one, change the other.

Usage:
    python parity_check.py --pt runs/detect/dartscore/weights/best.pt \
        --weights ../models/dartscore.safetensors  sample1.jpg sample2.jpg
"""
import argparse
import json
import math
import subprocess
import sys
from pathlib import Path

from ultralytics import YOLO


def ultra_dets(model, img: Path, conf: float, imgsz: int, device: str) -> list[dict]:
    res = model.predict(str(img), conf=conf, imgsz=imgsz, device=device, verbose=False)[0]
    return [
        {"class": int(b.cls), "cx": float(b.xywh[0][0]), "cy": float(b.xywh[0][1])}
        for b in res.boxes
    ]


def candle_dets(bin_cmd: list[str], weights: Path, img: Path) -> list[dict]:
    cmd = bin_cmd + ["detect", "--json", "--weights", str(weights), str(img)]
    p = subprocess.run(cmd, capture_output=True, text=True)
    if p.returncode != 0:
        raise SystemExit(f"candle detect failed on {img}:\n{p.stderr.strip()}")
    data = json.loads(p.stdout)
    return [{"class": d["class"], "cx": d["cx"], "cy": d["cy"]} for d in data["detections"]]


def nearest_unused(d: dict, candidates: list[dict]) -> tuple[dict | None, float]:
    best, best_dist = None, float("inf")
    for o in candidates:
        if o["class"] != d["class"] or o.get("_used"):
            continue
        dist = math.hypot(d["cx"] - o["cx"], d["cy"] - o["cy"])
        if dist < best_dist:
            best, best_dist = o, dist
    return best, best_dist


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--pt", required=True, type=Path, help="Ultralytics .pt (reference)")
    ap.add_argument("--weights", required=True, type=Path, help="candle .safetensors")
    ap.add_argument("--conf", type=float, default=0.25)
    ap.add_argument("--imgsz", type=int, default=640, help="must equal candle IMG_SIZE")
    ap.add_argument("--tol", type=float, default=2.0, help="max pixel error to pass")
    ap.add_argument("--device", default="cpu",
                    help="Ultralytics device; cpu matches candle (CPU) and avoids ROCm")
    ap.add_argument("--bin", default="cargo run --quiet --features detection --",
                    help="command that runs the Rust binary")
    ap.add_argument("images", nargs="+", type=Path)
    args = ap.parse_args()

    model = YOLO(str(args.pt))
    bin_cmd = args.bin.split()
    worst, total_unmatched, failed = 0.0, 0, False

    for img in args.images:
        ref = ultra_dets(model, img, args.conf, args.imgsz, args.device)
        got = candle_dets(bin_cmd, args.weights, img)
        img_worst, missed = 0.0, 0
        for d in ref:
            match, dist = nearest_unused(d, got)
            if match is None:
                missed += 1
            else:
                match["_used"] = True
                img_worst = max(img_worst, dist)
        extra = sum(1 for o in got if not o.get("_used"))
        total_unmatched += missed + extra
        worst = max(worst, img_worst)
        ok = img_worst <= args.tol and missed == 0 and extra == 0
        failed |= not ok
        print(f"{img.name}: {len(ref)} ref / {len(got)} candle, "
              f"max err {img_worst:.2f}px, missed={missed} extra={extra} "
              f"[{'OK' if ok else 'FAIL'}]")

    print(f"\nworst error {worst:.2f}px (tol {args.tol}px), "
          f"total unmatched {total_unmatched} -> {'PARITY OK' if not failed else 'PARITY FAILED'}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
