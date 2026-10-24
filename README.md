# Handy Server

Headless speech-to-text server forked from [cjpais/Handy](https://github.com/cjpais/Handy). Upload a recording, get Korean text with timestamps. Default model: whisper-large-v3-turbo (downloaded on first run).

```bash
cd src-tauri && cargo build --release
./target/release/handy            # listens on 127.0.0.1:8080
curl -F file=@meeting.m4a localhost:8080/transcribe
```

Response: `{"text", "language":"ko", "duration_sec", "segments":[{"start","end","text"}]}` (seconds).

Env: `HANDY_ADDR` (default 127.0.0.1:8080), `HANDY_DATA_DIR` (default ~/.handy-server), `HANDY_RESOURCE_DIR`, `HANDY_API_TOKEN` (optional Bearer auth; `/health` stays open).
