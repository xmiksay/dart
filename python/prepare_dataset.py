#!/usr/bin/env python3
"""Convert the DeepDarts annotations into a YOLOv8 detection dataset.

DeepDarts models keypoints-as-objects: each image has up to 7 normalized (x, y)
points — the first 4 are board calibration points, the rest (≤3) are dart tips.
YOLOv8 detection wants boxes, so we emit a small fixed-size box centred on each
point, with the frozen 5-class layout (see data.yaml):

    0 cal_top  1 cal_right  2 cal_bottom  3 cal_left  4 dart

CRITICAL — calibration order: the model's classes 0–3 must correspond to the
board corners 5&20 (top), 13&6 (right), 17&3 (bottom), 8&11 (left), in that
order, to match src/geometry.rs. DeepDarts' first-4 order may differ; if so, set
`--calib-order` to permute them, and ALWAYS run `--visualize <dir>` once to
eyeball that cal_top really is the top corner, etc.

Annotations live in a pandas pickle (``labels.pkl``); column names vary between
releases, so we detect them and log what we used.

Usage:
    python prepare_dataset.py --pkl labels.pkl --img-root images \
        --out datasets/dartscore --visualize viz/   # then check viz/*.jpg
    # if the corners are mislabelled, re-run with e.g. --calib-order 0,3,2,1
"""
import argparse
import random
import shutil
from pathlib import Path

import cv2
import numpy as np
import pandas as pd

CLASS_NAMES = ["cal_top", "cal_right", "cal_bottom", "cal_left", "dart"]
CLASS_COLORS = [(0, 0, 255), (0, 255, 0), (255, 0, 0), (0, 255, 255), (255, 0, 255)]


def find_column(df: pd.DataFrame, candidates: list[str]) -> str:
    for c in candidates:
        if c in df.columns:
            return c
    raise SystemExit(
        f"none of {candidates} found in pickle columns: {list(df.columns)}"
    )


def normalized_points(raw) -> np.ndarray:
    """Coerce a row's annotation into an (N, 2) float array in [0, 1]."""
    arr = np.asarray(raw, dtype=float)
    if arr.ndim == 1:
        arr = arr.reshape(-1, 2)
    return arr


def labels_for(points: np.ndarray, box: float, calib_order: list[int]) -> list[str]:
    """YOLO label lines for one image; calib points first, then darts."""
    lines = []
    for i, (x, y) in enumerate(points):
        cls = calib_order[i] if i < 4 else 4
        # Skip missing/occluded points encoded as NaN or (0, 0).
        if not (np.isfinite(x) and np.isfinite(y)):
            continue
        if x <= 0.0 and y <= 0.0:
            continue
        x = min(max(x, 0.0), 1.0)
        y = min(max(y, 0.0), 1.0)
        lines.append(f"{cls} {x:.6f} {y:.6f} {box:.6f} {box:.6f}")
    return lines


def draw_visualization(src: Path, points: np.ndarray, box: float,
                       calib_order: list[int], dst: Path) -> None:
    """Save a copy of the image with each labelled box + class name drawn."""
    img = cv2.imread(str(src))
    if img is None:
        return
    h, w = img.shape[:2]
    for i, (nx, ny) in enumerate(points):
        if not (np.isfinite(nx) and np.isfinite(ny)) or (nx <= 0 and ny <= 0):
            continue
        cls = calib_order[i] if i < 4 else 4
        cx, cy = int(nx * w), int(ny * h)
        half = int(box * w / 2)
        color = CLASS_COLORS[cls]
        cv2.rectangle(img, (cx - half, cy - half), (cx + half, cy + half), color, 2)
        cv2.putText(img, CLASS_NAMES[cls], (cx + half + 2, cy),
                    cv2.FONT_HERSHEY_SIMPLEX, 0.6, color, 2)
    cv2.imwrite(str(dst), img)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--pkl", required=True, type=Path)
    ap.add_argument("--img-root", required=True, type=Path)
    ap.add_argument("--out", default=Path("datasets/dartscore"), type=Path)
    ap.add_argument("--box-size", type=float, default=0.027,
                    help="normalized width/height of each point's box")
    ap.add_argument("--val-frac", type=float, default=0.1)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--calib-order", default="0,1,2,3",
                    help="class for each of the first 4 points (top,right,bottom,left)")
    ap.add_argument("--visualize", type=Path, default=None,
                    help="dir to write a few annotated images for verification")
    ap.add_argument("--viz-count", type=int, default=8)
    args = ap.parse_args()

    calib_order = [int(x) for x in args.calib_order.split(",")]
    if sorted(calib_order) != [0, 1, 2, 3]:
        raise SystemExit(f"--calib-order must be a permutation of 0,1,2,3: {calib_order}")

    df = pd.read_pickle(args.pkl)
    print(f"loaded {len(df)} rows; columns: {list(df.columns)}")

    xy_col = find_column(df, ["xy", "kps", "keypoints", "points"])
    name_col = find_column(df, ["img_name", "name", "filename"])
    folder_col = next((c for c in ["img_folder", "folder"] if c in df.columns), None)
    split_col = next((c for c in ["split", "set"] if c in df.columns), None)
    print(f"using columns: xy={xy_col} name={name_col} "
          f"folder={folder_col} split={split_col}; calib_order={calib_order}")

    rng = random.Random(args.seed)
    for sub in ("train", "val", "test"):
        (args.out / "images" / sub).mkdir(parents=True, exist_ok=True)
        (args.out / "labels" / sub).mkdir(parents=True, exist_ok=True)
    if args.visualize:
        args.visualize.mkdir(parents=True, exist_ok=True)

    counts = {"train": 0, "val": 0, "test": 0}
    boxes_total = 0
    viz_done = 0
    for _, row in df.iterrows():
        name = str(row[name_col])
        src = args.img_root / (str(row[folder_col]) if folder_col else "") / name
        if not src.exists():
            src = args.img_root / name  # fall back to a flat layout
        if not src.exists():
            print(f"  ! missing image, skipping: {src}")
            continue

        if split_col and str(row[split_col]) in ("train", "val", "test"):
            sub = str(row[split_col])
        else:
            sub = "val" if rng.random() < args.val_frac else "train"

        points = normalized_points(row[xy_col])
        lines = labels_for(points, args.box_size, calib_order)
        stem = Path(name).stem
        (args.out / "labels" / sub / f"{stem}.txt").write_text(
            "\n".join(lines) + ("\n" if lines else "")
        )
        shutil.copy(src, args.out / "images" / sub / name)
        counts[sub] += 1
        boxes_total += len(lines)

        if args.visualize and viz_done < args.viz_count:
            draw_visualization(src, points, args.box_size, calib_order,
                               args.visualize / f"{stem}.jpg")
            viz_done += 1

    print(f"done: {counts}, {boxes_total} boxes total")
    print(f"dataset written to {args.out} — point data.yaml `path:` here")
    if args.visualize:
        print(f"VERIFY: open {args.visualize}/*.jpg — cal_top must be the TOP "
              f"corner (5&20), cal_right the RIGHT (13&6), etc. If not, re-run "
              f"with a corrected --calib-order.")


if __name__ == "__main__":
    main()
