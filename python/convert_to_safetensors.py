#!/usr/bin/env python3
"""Convert an Ultralytics YOLOv8 .pt checkpoint to candle-compatible safetensors.

candle does NOT run ONNX or a saved graph — it reimplements the YOLOv8
architecture in Rust and loads weights by tensor *name*. So the Ultralytics
state-dict keys (``model.<idx>.…``) must be renamed to the names candle's
VarBuilder requests (``net.bN.…`` / ``fpn.nN.…`` / ``head.…``). This is the one
place the "just swap the file" boundary needs attention.

The index→name map below matches the candle `yolo-v8` example model definition.
After converting, run a parity check (verify_detection.py + the Rust `detect`
command) — if candle reports a missing/!shape tensor, reconcile the name here
against src/detect.rs. Authoritative reference: the lmz/candle-yolo-v8 HF repo
discussion.

Usage:
    python convert_to_safetensors.py --pt runs/detect/dartscore/weights/best.pt \
        --out ../models/dartscore.safetensors
"""
import argparse
from pathlib import Path

import torch
from safetensors.torch import save_file

# Ultralytics module index -> candle module path. Weightless layers (upsample,
# concat) have no parameters and so never appear as keys.
INDEX_MAP = {
    0: "net.b1.0", 1: "net.b1.1",
    2: "net.b2.0", 3: "net.b2.1", 4: "net.b2.2",
    5: "net.b3.0", 6: "net.b3.1",
    7: "net.b4.0", 8: "net.b4.1",
    9: "net.b5.0",
    12: "fpn.n1", 15: "fpn.n2", 16: "fpn.n3",
    18: "fpn.n4", 19: "fpn.n5", 21: "fpn.n6",
    22: "head",
}


def rename(key: str) -> str | None:
    """Map an Ultralytics key to a candle key, or None to drop it."""
    if key.endswith("num_batches_tracked"):
        return None
    if not key.startswith("model."):
        return None
    parts = key.split(".")
    idx = int(parts[1])
    if idx not in INDEX_MAP:
        return None  # weightless / unmapped layer
    rest = ".".join(parts[2:])
    # C2f's inner Bottleneck ModuleList is `.m.` in Ultralytics, `.bottleneck.`
    # in candle.
    rest = rest.replace(".m.", ".bottleneck.")
    return f"{INDEX_MAP[idx]}.{rest}"


def load_state_dict(pt_path: Path) -> dict:
    ckpt = torch.load(pt_path, map_location="cpu", weights_only=False)
    model = ckpt["model"] if isinstance(ckpt, dict) and "model" in ckpt else ckpt
    if hasattr(model, "state_dict"):
        return model.float().state_dict()
    return model


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--pt", required=True, type=Path)
    ap.add_argument("--out", required=True, type=Path)
    args = ap.parse_args()

    sd = load_state_dict(args.pt)
    out, dropped = {}, []
    for k, v in sd.items():
        nk = rename(k)
        if nk is None:
            dropped.append(k)
            continue
        out[nk] = v.contiguous().to(torch.float32)

    args.out.parent.mkdir(parents=True, exist_ok=True)
    save_file(out, str(args.out))
    print(f"wrote {len(out)} tensors -> {args.out}")
    if dropped:
        print(f"dropped {len(dropped)} keys (weightless/untracked); "
              f"sample: {dropped[:5]}")
    print("NEXT: run the Rust parity check — `cargo run --features detection -- "
          "detect <img>` must match verify_detection.py within ~1 px.")


if __name__ == "__main__":
    main()
