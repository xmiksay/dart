#!/usr/bin/env python3
"""Golden-reference detector: run the trained YOLOv8n on images and print the
detected points. This is the parity baseline the Rust/candle runtime (Phase 1)
must reproduce within ~1 px on the same input.

Usage:
    python verify_detection.py --weights runs/detect/dartscore/weights/best.pt img1.jpg ...
    python verify_detection.py --weights best.pt --json img1.jpg     # machine-readable
"""
import argparse
import json
from pathlib import Path

from ultralytics import YOLO

CLASS_NAMES = {0: "cal_top", 1: "cal_right", 2: "cal_bottom", 3: "cal_left", 4: "dart"}


def detect(model, img: Path, conf: float, imgsz: int) -> list[dict]:
    """Run the model on one image → list of {class, name, cx, cy, conf}."""
    res = model.predict(str(img), conf=conf, imgsz=imgsz, verbose=False)[0]
    dets = []
    for box in res.boxes:
        cls = int(box.cls)
        cx, cy = box.xywh[0][:2].tolist()  # box centre = the point
        dets.append({
            "class": cls,
            "name": CLASS_NAMES.get(cls, str(cls)),
            "cx": cx,
            "cy": cy,
            "conf": float(box.conf),
        })
    return dets


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--weights", required=True, type=Path)
    ap.add_argument("--conf", type=float, default=0.25)
    ap.add_argument("--imgsz", type=int, default=800)
    ap.add_argument("--json", action="store_true", help="emit JSON (for the harness)")
    ap.add_argument("images", nargs="+", type=Path)
    args = ap.parse_args()

    model = YOLO(str(args.weights))
    for img in args.images:
        dets = detect(model, img, args.conf, args.imgsz)
        if args.json:
            print(json.dumps({"image": str(img), "detections": dets}))
        else:
            print(f"\n{img.name}:")
            for d in dets:
                print(f"  {d['name']:10s} ({d['cx']:7.2f}, {d['cy']:7.2f}) conf={d['conf']:.3f}")


if __name__ == "__main__":
    main()
