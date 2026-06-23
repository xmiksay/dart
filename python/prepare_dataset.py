#!/usr/bin/env python3
"""Convert the DeepDarts annotations into a YOLOv8 detection dataset.

DeepDarts models keypoints-as-objects: each image has up to 7 normalized (x, y)
points — the first 4 are board calibration points, the rest (≤3) are dart tips.
YOLOv8 detection wants boxes, so we emit a small fixed-size box centred on each
point, with the frozen 5-class layout (see data.yaml):

    0 cal_top  1 cal_right  2 cal_bottom  3 cal_left  4 dart

The original annotations live in a pandas pickle (``labels.pkl``) with one row per
image. Column names vary slightly between releases, so we detect them. Verify the
detected columns in the log output against your copy of the dataset.

Usage:
    python prepare_dataset.py --pkl path/to/labels.pkl --img-root path/to/images \
        --out datasets/dartscore --box-size 0.027 --val-frac 0.1
"""
import argparse
import random
import shutil
from pathlib import Path

import numpy as np
import pandas as pd


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


def write_label(label_path: Path, points: np.ndarray, box: float) -> int:
    """Write a YOLO label file. Returns the number of boxes written."""
    lines = []
    for i, (x, y) in enumerate(points):
        # Calibration points are the first four; everything after is a dart tip.
        if i < 4:
            cls = i
        else:
            cls = 4
        # Skip missing/occluded points encoded as NaN or (0, 0).
        if not (np.isfinite(x) and np.isfinite(y)):
            continue
        if x <= 0.0 and y <= 0.0:
            continue
        x = min(max(x, 0.0), 1.0)
        y = min(max(y, 0.0), 1.0)
        lines.append(f"{cls} {x:.6f} {y:.6f} {box:.6f} {box:.6f}")
    label_path.write_text("\n".join(lines) + ("\n" if lines else ""))
    return len(lines)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--pkl", required=True, type=Path)
    ap.add_argument("--img-root", required=True, type=Path)
    ap.add_argument("--out", default=Path("datasets/dartscore"), type=Path)
    ap.add_argument("--box-size", type=float, default=0.027,
                    help="normalized width/height of each point's box")
    ap.add_argument("--val-frac", type=float, default=0.1)
    ap.add_argument("--seed", type=int, default=0)
    args = ap.parse_args()

    df = pd.read_pickle(args.pkl)
    print(f"loaded {len(df)} rows; columns: {list(df.columns)}")

    xy_col = find_column(df, ["xy", "kps", "keypoints", "points"])
    name_col = find_column(df, ["img_name", "name", "filename"])
    folder_col = next((c for c in ["img_folder", "folder"] if c in df.columns), None)
    split_col = next((c for c in ["split", "set"] if c in df.columns), None)
    print(f"using columns: xy={xy_col} name={name_col} "
          f"folder={folder_col} split={split_col}")

    rng = random.Random(args.seed)
    for sub in ("train", "val", "test"):
        (args.out / "images" / sub).mkdir(parents=True, exist_ok=True)
        (args.out / "labels" / sub).mkdir(parents=True, exist_ok=True)

    counts = {"train": 0, "val": 0, "test": 0}
    boxes_total = 0
    for _, row in df.iterrows():
        name = str(row[name_col])
        src = args.img_root / (str(row[folder_col]) if folder_col else "") / name
        if not src.exists():
            # Fall back to a flat layout.
            src = args.img_root / name
        if not src.exists():
            print(f"  ! missing image, skipping: {src}")
            continue

        if split_col and str(row[split_col]) in ("train", "val", "test"):
            sub = str(row[split_col])
        else:
            sub = "val" if rng.random() < args.val_frac else "train"

        points = normalized_points(row[xy_col])
        stem = Path(name).stem
        n = write_label(args.out / "labels" / sub / f"{stem}.txt", points, args.box_size)
        shutil.copy(src, args.out / "images" / sub / name)
        counts[sub] += 1
        boxes_total += n

    print(f"done: {counts}, {boxes_total} boxes total")
    print(f"dataset written to {args.out} — point data.yaml `path:` here")


if __name__ == "__main__":
    main()
