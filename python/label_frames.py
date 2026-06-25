#!/usr/bin/env python3
"""Click-to-label tool for a FIXED, calibrated camera.

The 4 board calibration corners don't move, so they're read from calibration.json
and written into every image's label automatically — you only click the dart
tip(s). Output is the standard YOLO 5-class layout (see data.yaml), ready for
train.py.

Controls (per image):
    left click   add a dart tip (at the point it enters the board)
    right click  undo the last tip
    c            clear all tips
    n / space    save this image + next
    s            skip (don't save)
    q            quit

Usage:
    python label_frames.py --images captures --calibration ../calibration.json \
        --out datasets/mine --box-size 0.027 --val-frac 0.1
"""
import argparse
import json
import random
import shutil
from pathlib import Path

try:
    import cv2
except ModuleNotFoundError:
    raise SystemExit(
        "opencv-python is required — `pip install -r requirements.txt` "
        "(or `pip install opencv-python`)"
    )

CLASS_NAMES = ["cal_top", "cal_right", "cal_bottom", "cal_left", "dart"]
CALIB_COLOR = (0, 200, 255)
TIP_COLOR = (0, 0, 255)
IMG_EXTS = {".png", ".jpg", ".jpeg", ".bmp"}


def load_calibration(path: Path) -> list[tuple[float, float]]:
    """The 4 corner pixels in canonical order: top, right, bottom, left."""
    data = json.loads(path.read_text())
    pts = [(float(p["x"]), float(p["y"])) for p in data["points"]]
    if len(pts) != 4:
        raise SystemExit(f"calibration.json must have 4 points, got {len(pts)}")
    return pts


def write_label(path: Path, calib, tips, w: int, h: int, box: float) -> None:
    lines = []
    for cls, (x, y) in enumerate(calib):  # 0..3 = the fixed corners
        lines.append(f"{cls} {x / w:.6f} {y / h:.6f} {box:.6f} {box:.6f}")
    for (x, y) in tips:  # class 4 = dart
        lines.append(f"4 {x / w:.6f} {y / h:.6f} {box:.6f} {box:.6f}")
    path.write_text("\n".join(lines) + "\n")


def draw(img, calib, tips, box_px: int):
    canvas = img.copy()
    for cls, (x, y) in enumerate(calib):
        p = (int(x), int(y))
        cv2.drawMarker(canvas, p, CALIB_COLOR, cv2.MARKER_CROSS, 24, 2)
        cv2.putText(canvas, CLASS_NAMES[cls], (p[0] + 8, p[1] - 8),
                    cv2.FONT_HERSHEY_SIMPLEX, 0.5, CALIB_COLOR, 1)
    for i, (x, y) in enumerate(tips):
        p = (int(x), int(y))
        cv2.circle(canvas, p, 5, TIP_COLOR, -1)
        cv2.rectangle(canvas, (p[0] - box_px, p[1] - box_px),
                      (p[0] + box_px, p[1] + box_px), TIP_COLOR, 1)
        cv2.putText(canvas, f"dart {i + 1}", (p[0] + 8, p[1]),
                    cv2.FONT_HERSHEY_SIMPLEX, 0.5, TIP_COLOR, 1)
    cv2.putText(canvas, "L:add  R:undo  c:clear  n/space:save  s:skip  q:quit",
                (10, 24), cv2.FONT_HERSHEY_SIMPLEX, 0.6, (255, 255, 255), 2)
    return canvas


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--images", required=True, type=Path, help="folder of frames to label")
    ap.add_argument("--calibration", default=Path("../calibration.json"), type=Path)
    ap.add_argument("--out", default=Path("datasets/dartscore"), type=Path,
                    help="dataset root (matches data.yaml `path:` by default)")
    ap.add_argument("--box-size", type=float, default=0.027)
    ap.add_argument("--val-frac", type=float, default=0.1)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--overwrite", action="store_true", help="re-label already-done images")
    args = ap.parse_args()

    calib = load_calibration(args.calibration)
    images = sorted(p for p in args.images.rglob("*") if p.suffix.lower() in IMG_EXTS)
    if not images:
        raise SystemExit(f"no images found under {args.images}")

    for sub in ("train", "val"):
        (args.out / "images" / sub).mkdir(parents=True, exist_ok=True)
        (args.out / "labels" / sub).mkdir(parents=True, exist_ok=True)

    rng = random.Random(args.seed)
    tips: list[tuple[int, int]] = []

    def on_mouse(event, x, y, _flags, _param):
        if event == cv2.EVENT_LBUTTONDOWN:
            tips.append((x, y))
        elif event == cv2.EVENT_RBUTTONDOWN and tips:
            tips.pop()

    win = "label (fixed-camera): click dart tips"
    cv2.namedWindow(win, cv2.WINDOW_NORMAL)
    cv2.setMouseCallback(win, on_mouse)

    saved, skipped = 0, 0
    for path in images:
        stem = path.stem
        done = any((args.out / "labels" / s / f"{stem}.txt").exists() for s in ("train", "val"))
        if done and not args.overwrite:
            continue
        img = cv2.imread(str(path))
        if img is None:
            print(f"  ! unreadable, skipping: {path}")
            continue
        h, w = img.shape[:2]
        box_px = int(args.box_size * w / 2)
        tips.clear()

        action = None
        while action is None:
            cv2.imshow(win, draw(img, calib, tips, box_px))
            key = cv2.waitKey(20) & 0xFF
            if key in (ord("n"), ord(" ")):
                action = "save"
            elif key == ord("s"):
                action = "skip"
            elif key == ord("c"):
                tips.clear()
            elif key == ord("q"):
                action = "quit"

        if action == "quit":
            break
        if action == "skip":
            skipped += 1
            continue

        sub = "val" if rng.random() < args.val_frac else "train"
        shutil.copy(path, args.out / "images" / sub / path.name)
        write_label(args.out / "labels" / sub / f"{stem}.txt", calib, tips, w, h, args.box_size)
        saved += 1
        print(f"saved {stem}: {len(tips)} dart(s) -> {sub}")

    cv2.destroyAllWindows()
    print(f"\ndone: {saved} labeled, {skipped} skipped")
    print(f"dataset at {args.out} — point a data.yaml `path:` here, then train.py")


if __name__ == "__main__":
    main()
