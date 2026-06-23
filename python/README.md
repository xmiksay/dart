# Detection training & conversion (Phase 0)

Offline toolchain that runs on the **dev machine with an NVIDIA GPU** — never on
the Pi. Only one artifact crosses to the Pi: `../models/dartscore.safetensors`.

The detector's class layout is **frozen** (`data.yaml`) and must stay in lockstep
with the Rust side (`src/detect.rs` `NUM_CLASSES`, `src/geometry.rs` calibration
order). candle reimplements the network head with this exact class count, so a
class change means a code change on both sides.

## Setup

```sh
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
```

## Pipeline

1. **Get DeepDarts** (dataset: ieee-dataport "DeepDarts"; annotations as a
   pandas `labels.pkl`). Or grab a ready YOLOv8 export (Roboflow Universe
   "DeepDarts YOLOv8") and skip step 2.

2. **Convert to YOLOv8 5-class** (keypoints → centred boxes):
   ```sh
   python prepare_dataset.py --pkl labels.pkl --img-root images \
       --out datasets/dartscore --box-size 0.027 --val-frac 0.1
   ```
   Check the logged column names match your pickle. Point `data.yaml`'s `path:`
   at the output dir.

3. **Train YOLOv8n** (local GPU):
   ```sh
   python train.py --data data.yaml --epochs 100 --imgsz 800 --batch 16
   ```
   → `runs/detect/dartscore/weights/best.pt`.

4. **Golden reference** (Phase 0 done when this finds calib points + tips):
   ```sh
   python verify_detection.py --weights runs/detect/dartscore/weights/best.pt sample.jpg
   ```

5. **Convert to safetensors** for candle:
   ```sh
   python convert_to_safetensors.py \
       --pt runs/detect/dartscore/weights/best.pt \
       --out ../models/dartscore.safetensors
   ```

6. **Parity gate (Phase 1)**: `cargo run --features detection -- detect sample.jpg`
   must match step 4 within ~1 px. If candle reports a missing/mismatched tensor,
   reconcile the layer-name map in `convert_to_safetensors.py` against
   `src/detect.rs`.
