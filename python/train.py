#!/usr/bin/env python3
"""Train YOLOv8n on the dartscore dataset (Ultralytics, local GPU).

The 5-class layout is frozen (see data.yaml). After training, export best.pt and
convert it with convert_to_safetensors.py for the candle runtime.

Usage:
    python train.py --data data.yaml --epochs 100 --imgsz 800 --batch 16
"""
import argparse
from pathlib import Path

from ultralytics import YOLO


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", default="data.yaml", type=Path)
    ap.add_argument("--model", default="yolov8n.pt",
                    help="base weights to fine-tune (COCO-pretrained yolov8n)")
    ap.add_argument("--epochs", type=int, default=100)
    # Match candle's IMG_SIZE (src/detect/mod.rs) so train = inference = parity.
    ap.add_argument("--imgsz", type=int, default=640)
    ap.add_argument("--batch", type=int, default=16)
    ap.add_argument("--device", default="0")
    ap.add_argument("--name", default="dartscore")
    args = ap.parse_args()

    model = YOLO(str(args.model))
    model.train(
        data=str(args.data),
        epochs=args.epochs,
        imgsz=args.imgsz,
        batch=args.batch,
        device=args.device,
        name=args.name,
    )
    # Report the ACTUAL save dir (Ultralytics auto-increments name on re-runs).
    best = Path(model.trainer.save_dir) / "weights" / "best.pt"
    print(f"\ntraining complete — best weights at {best}")
    print(f"convert it:  python convert_to_safetensors.py --pt {best} "
          f"--out ../models/dartscore.safetensors")


if __name__ == "__main__":
    main()
