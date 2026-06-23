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
    ap.add_argument("--imgsz", type=int, default=800)
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
    # Ultralytics writes runs/detect/<name>/weights/best.pt
    print("training complete — best weights at "
          f"runs/detect/{args.name}/weights/best.pt")


if __name__ == "__main__":
    main()
