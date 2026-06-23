#!/usr/bin/env python3
"""Golden-reference detector: run the trained YOLOv8n on images and print the
detected points. This is the parity baseline the Rust/candle runtime (Phase 1)
must reproduce within ~1 px on the same input.

Usage:
    python verify_detection.py --weights runs/detect/dartscore/weights/best.pt img1.jpg ...
"""
import argparse
from pathlib import Path

from ultralytics import YOLO

CLASS_NAMES = {0: "cal_top", 1: "cal_right", 2: "cal_bottom", 3: "cal_left", 4: "dart"}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--weights", required=True, type=Path)
    ap.add_argument("--conf", type=float, default=0.25)
    ap.add_argument("--imgsz", type=int, default=800)
    ap.add_argument("images", nargs="+", type=Path)
    args = ap.parse_args()

    model = YOLO(str(args.weights))
    for img in args.images:
        res = model.predict(str(img), conf=args.conf, imgsz=args.imgsz, verbose=False)[0]
        print(f"\n{img.name}:")
        for box in res.boxes:
            cls = int(box.cls)
            cx, cy = box.xywh[0][:2].tolist()  # box centre = the point
            conf = float(box.conf)
            print(f"  {CLASS_NAMES.get(cls, cls):10s} ({cx:7.2f}, {cy:7.2f}) conf={conf:.3f}")


if __name__ == "__main__":
    main()
