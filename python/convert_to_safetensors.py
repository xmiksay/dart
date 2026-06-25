#!/usr/bin/env python3
"""Convert an Ultralytics YOLOv8 .pt checkpoint to candle-compatible safetensors.

candle does NOT run ONNX or a saved graph — it reimplements the YOLOv8
architecture in Rust and loads weights by tensor *name*. So the Ultralytics
state-dict keys (``model.<idx>.…``) must be renamed to the names candle's
VarBuilder requests (``net.bN.…`` / ``fpn.nN.…`` / ``head.…``). This is the one
place the "just swap the file" boundary needs attention.

The index→name map below matches the vendored candle model (src/detect/model.rs):
backbone `net.bN.M`, neck `fpn.nN` (Ultralytics indices 12/15/16/18/19/21), head
`head.*`, and C2f's inner Bottleneck list `.m.` → `.bottleneck.`. ConvBlocks keep
both `conv.weight` and `bn.*` — candle folds the BN at load time (`absorb_bn`), so
both must be present and `num_batches_tracked` is dropped.

After converting, **validate it loads into candle** (the authoritative check):
    cargo run --features detection -- detect --check --weights ../models/dartscore.safetensors
If candle reports a missing/shape-mismatched tensor, reconcile the name here.

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
    full = f"{INDEX_MAP[idx]}." + ".".join(parts[2:])
    # C2f's inner Bottleneck ModuleList is `.m.` in Ultralytics, `.bottleneck.`
    # in candle. Replace on the FULL key so the leading `m.` (e.g. b2.0.m.0)
    # has a preceding dot and actually matches.
    return full.replace(".m.", ".bottleneck.")


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
    out, dropped, suspicious = {}, [], []
    for k, v in sd.items():
        nk = rename(k)
        if nk is None:
            dropped.append(k)
            # A real weight (not num_batches_tracked) on a *mapped* layer that
            # still dropped means a naming gap — surface it loudly.
            if k.startswith("model.") and not k.endswith("num_batches_tracked"):
                parts = k.split(".")
                if len(parts) > 1 and parts[1].isdigit() and int(parts[1]) in INDEX_MAP:
                    suspicious.append(k)
            continue
        out[nk] = v.contiguous().to(torch.float32)

    if not out:
        raise SystemExit(
            "no tensors mapped — is this an Ultralytics YOLOv8 .pt? "
            f"sample keys: {list(sd)[:5]}"
        )

    args.out.parent.mkdir(parents=True, exist_ok=True)
    save_file(out, str(args.out))
    print(f"wrote {len(out)} tensors -> {args.out}")
    print(f"dropped {len(dropped)} keys (weightless / num_batches_tracked)")
    if suspicious:
        print(f"WARNING: {len(suspicious)} real weights on mapped layers were "
              f"dropped (naming gap?): {suspicious[:8]}")
    print(
        "NEXT: validate it loads into candle —\n"
        f"  cargo run --features detection -- detect --check --weights {args.out}\n"
        "then check parity:  python parity_check.py --weights <best.pt> <images...>"
    )


if __name__ == "__main__":
    main()
