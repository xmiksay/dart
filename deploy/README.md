# Deploying dartscore to a Raspberry Pi (arm64)

The release is a **single binary** (the Vue SPA is embedded via rust-embed), plus
a Postgres database. Cross-compile on your dev machine, copy to the Pi, run it
under systemd.

## 1. Build on the dev machine

Requires [`cross`](https://github.com/cross-rs/cross) (`cargo install cross`) and
Docker/Podman; the frontend is built first so it can be embedded.

```sh
# RPi 5 (cortex-a76) — RPi 4 (cortex-a72) — RPi 3 (cortex-a53)
make pi-package ARM_CPU=cortex-a72
# → dist/pi/  (dartscore binary + both systemd units + dartscore.env.example + this README)
```
A `cortex-a72` binary also runs on a Pi 5; a `cortex-a76` one **SIGILLs** on a Pi 4
(a76 is ARMv8.2, a72 is ARMv8.0). Build a72 if one image must cover both.

Or push it straight to the Pi over SSH:
```sh
make pi-deploy PI=pi@raspberrypi.local      # rsyncs dist/pi/ to /opt/dartscore
```

## 2. One-time setup on the Pi

```sh
# Runtime deps. The binary itself links only libc/libm/libgcc — `v4l2-ctl`
# (v4l-utils) is what the exposure/control locking shells out to.
# (No libv4l needed: nokhwa's v4l backend uses raw ioctls, and Debian 13
# dropped the `libv4l-0` name in the t64 transition anyway.)
sudo apt install postgresql v4l-utils

# Database + role:
sudo -u postgres psql -c "CREATE USER dartscore PASSWORD 'dartscore';"
sudo -u postgres psql -c "CREATE DATABASE dartscore OWNER dartscore;"

# Service account + install dir:
sudo useradd --system --home /opt/dartscore --shell /usr/sbin/nologin dartscore
sudo usermod -aG video dartscore            # camera access
sudo mkdir -p /opt/dartscore && sudo chown dartscore:dartscore /opt/dartscore
```

Copy the artifacts (if you used `pi-package`, scp `dist/pi/*` to the Pi):
```sh
sudo install -o dartscore -g dartscore -m755 dartscore /opt/dartscore/dartscore
sudo install -m640 dartscore.env.example /etc/dartscore.env   # then EDIT it
sudo install -m644 dartscore.service /etc/systemd/system/dartscore.service
```

Put your `calibration.json` and (once trained) `models/dartscore.safetensors`
into `/opt/dartscore/` — the server reads them relative to its WorkingDirectory.

## 3. Run

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now dartscore
systemctl status dartscore
journalctl -u dartscore -f                  # logs (camera/trigger/throws)
```

Open `http://<pi>:8080` from any browser on the LAN. For the projector itself,
use the kiosk unit below (or "install" the PWA: browser → Install app).

## 4. Projector kiosk (optional, RPi OS Lite)

`dartscore-kiosk.service` runs Chromium fullscreen on `tty1` under `cage`, a
minimal Wayland kiosk compositor — no desktop environment required.

```sh
sudo apt install --no-install-recommends cage chromium
sudo useradd -m -s /bin/bash kiosk
sudo usermod -aG video,render,input kiosk
sudo install -m644 dartscore-kiosk.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now dartscore-kiosk
```

The unit takes `tty1` from the getty (`Conflicts=getty@tty1.service`) and gets its
logind seat via `PAMName=login` — no `seatd` needed on a systemd host.

**It refuses to start unless an HDMI output is connected**, by design: with no
connected output wlroots has no vblank to throttle against and cage + chromium
free-run at ~100% of a core. The `ExecStartPre` connector check makes it fail fast
and give up after five attempts instead. Plug the projector in *before* boot, or
`sudo systemctl start dartscore-kiosk` once it's attached.

## Notes
- Migrations run automatically on startup; the live game survives restarts
  (snapshotted to Postgres).
- No camera? Leave `DART_CAMERA` unset in `/etc/dartscore.env` — the web game
  works with manual scoring.
- RPi 3 CPU inference is slow (seconds/frame) but functional; RPi 5 is the target.
- Update = rebuild (`make pi-deploy`) then `sudo systemctl restart dartscore`.

## Troubleshooting

- **`captures/ Permission denied`** — the saved-frame dir is created under the
  WorkingDirectory and must be writable by the service user. Frame-saving is
  best-effort (it won't kill detection), but to keep training frames:
  `sudo chown -R dartscore:dartscore /opt/dartscore`.
- **`GET /api/camera/snapshot … 503` / browser Calibrate shows nothing** — only
  happens with `DART_CAMERA` set *and* before the feed has published its first
  frame (~0.5 s after start), or if the camera failed to open. With the feed
  running, the snapshot endpoint serves the feed's latest frame (it can't open the
  busy device a second time); check `journalctl -u dartscore` for a capture-loop
  error if it persists.
- **`v4l2-ctl --set-ctrl gain=… failed: unknown control`** — harmless: the camera
  lacks that knob. dartscore logs unsupported controls at debug and continues.
  Set only controls your camera reports (`v4l2-ctl -d /dev/videoN --list-ctrls`).
- **Camera resets to auto on open** — v4l2 controls set externally before the
  stream opens are lost; dartscore re-applies exposure/controls itself after
  opening. Pass exposure via `DART_EXPOSURE` (a multiple of 10 ms, e.g. 200, to
  avoid 50 Hz mains flicker), extra knobs via the capture CLI `--ctrl name=value`.
