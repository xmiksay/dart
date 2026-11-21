# Detection training & conversion (Phase 0/1)

Offline toolchain that runs on the **dev machine with an NVIDIA GPU** — never on
the Pi. Only one artifact crosses to the Pi: `../models/dartscore.safetensors`.

The detector's class layout is **frozen** (`data.yaml`) and must stay in lockstep
with the Rust side (`src/detect/mod.rs` `NUM_CLASSES`/`CLASS_NAMES`,
`src/geometry.rs` calibration order). candle reimplements the network head with
this exact class count, so a class change means a code change on both sides.

**Keep input size consistent:** candle's `IMG_SIZE` is **640** (src/detect/mod.rs)
and confidence is 0.25 — train and parity-check at 640 too (the defaults here).

## Setup

Install PyTorch for your GPU **first** (so the right build isn't clobbered), then
the rest:

Use **Python 3.12** (3.13/3.14 have no PyTorch wheels). miniforge isolates it
from the system Python cleanly:

```sh
conda create -n dart python=3.12 -y && conda activate dart

# torch AND torchvision together, from the SAME index (matched versions):
# AMD Radeon / ROCm (Linux):
pip install torch torchvision --index-url https://download.pytorch.org/whl/rocm6.2
# (NVIDIA: .../cu124  ·  CPU only: .../cpu)

python -c "import torchvision; torchvision.ops.nms; import torch; print(torch.cuda.is_available())"
pip install -r requirements.txt
```

Pitfalls: a torch/torchvision version mismatch → `operator torchvision::nms does
not exist`; mixing a pacman (system) torch with a pip torchvision causes the same.
Keep them paired in one clean env. ROCm note: consumer Radeons sometimes need
`export HSA_OVERRIDE_GFX_VERSION=10.3.0` (RDNA2) / `11.0.0` (RDNA3). Train with
`--device 0`; `--device cpu` is the fallback.

## Pipeline

1. **Get DeepDarts** (ieee-dataport "DeepDarts", annotations as a pandas
   `labels.pkl`). Or grab a ready YOLOv8 export (Roboflow "DeepDarts YOLOv8").

2. **Convert to the 5-class YOLO layout — and *verify the corner order*:**
   ```sh
   python prepare_dataset.py --pkl labels.pkl --img-root images \
       --out datasets/dartscore --visualize viz/
   ```
   Open `viz/*.jpg`: **cal_top** must sit on the top corner (5&20), **cal_right**
   on the right (13&6), etc. If they're permuted, re-run with e.g.
   `--calib-order 0,3,2,1`. Then point `data.yaml`'s `path:` at the output dir.
   (This order is the #1 thing that silently breaks the homography.)

3. **Train YOLOv8n** (local GPU):
   ```sh
   python train.py --data data.yaml --epochs 100 --imgsz 640 --batch 16
   ```
   → `runs/detect/dartscore/weights/best.pt`.

4. **Golden reference** (Phase 0 done when this finds calib points + tips):
   ```sh
   python verify_detection.py --weights runs/detect/dartscore/weights/best.pt sample.jpg
   ```

5. **Convert to safetensors** for candle:
   ```sh
   python convert_to_safetensors.py \
       --pt runs/detect/dartscore/weights/best.pt --out ../models/dartscore.safetensors
   ```

6. **Validate it loads into candle** (catches any layer-name mismatch):
   ```sh
   cargo run --features detection -- detect --check --weights ../models/dartscore.safetensors
   ```

7. **Parity gate (Phase 1)** — candle must reproduce Ultralytics within tolerance:
   ```sh
   python parity_check.py --pt runs/detect/dartscore/weights/best.pt \
       --weights ../models/dartscore.safetensors  sample1.jpg sample2.jpg
   ```
   Exit 0 = parity holds (default ≤ 2 px). If it fails on *names*, fix the map in
   `convert_to_safetensors.py`; if on *positions*, check `IMG_SIZE`/`--imgsz` match.

Then drop `models/dartscore.safetensors` on the Pi — `make capture` / `serve`
auto-switch to the model (no code change), and per-frame calibration makes it
tolerant of a non-rigid camera.

## Troubleshooting (environment / GPU)

- **`No matching distribution found for torch`** — your Python is too new. PyTorch
  ships wheels up to ~3.12; 3.13/3.14 have none. Use a 3.12 env (miniforge, `uv
  venv --python 3.12`, or pyenv).
- **`operator torchvision::nms does not exist`** — torch/torchvision version
  mismatch. Install them **together from the same index**, and don't mix a system
  (pacman) torch with a pip torchvision.
- **AMD ROCm, `HIP error: invalid device function` / `rocBLAS … TensileLibrary …
  gfxNNNN`** — ROCm has no kernels for your exact GPU arch. Set the nearest
  supported one: RDNA3 (gfx110x, e.g. **gfx1101**) → `export
  HSA_OVERRIDE_GFX_VERSION=11.0.0`; RDNA2 (gfx103x) → `10.3.0`. Find your arch with
  `rocminfo | grep -i gfx`. Persist it in `~/.bashrc`.
- **`amdgpu.ids: No such file or directory`** — harmless (libdrm GPU-name DB);
  `sudo pacman -S libdrm hwids` if the generic "AMD Radeon Graphics" name bothers you.
- **`parity_check.py` aborts on rocBLAS** — it runs the reference on **CPU** by
  default (`--device cpu`), matching candle (also CPU), so it doesn't need ROCm.
- **Conversion** (`convert_to_safetensors.py`) is CPU-only — no GPU needed.

## Collecting & labeling your own frames (fixed camera)

For best accuracy on *your* board, fine-tune on frames from your own camera. The
fixed-camera trick: the 4 calibration corners never move, so they come from
`calibration.json` automatically and you only click the dart tips.

1. **Capture varied frames** (all segments, 1–3 darts, some empty, overlaps):
   ```sh
   make capture CAMERA=0 EXPOSURE=200 INTERVAL=2     # frames land in captures/
   ```
2. **Label them** — click each dart tip; the 4 corners are stamped in for you:
   ```sh
   python label_frames.py --images ../captures --calibration ../calibration.json \
       --out datasets/mine --box-size 0.027 --val-frac 0.1
   ```
   (left-click = add tip, right-click = undo, `n`/space = save+next, `q` = quit.)
   This writes the standard YOLO 5-class layout under `datasets/mine`.
3. **Train / fine-tune** on it (point `data.yaml` `path:` at `datasets/mine`, or
   merge it with DeepDarts for robustness + your-board accuracy):
   ```sh
   python train.py --data data.yaml --imgsz 640
   ```
   Best results: train on DeepDarts first, then fine-tune on your frames.
