#!/usr/bin/env python3

import os
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


class AudioHandler(BaseHTTPRequestHandler):
    audio = b""

    def do_POST(self) -> None:
        length = int(self.headers.get("Content-Length", "0"))
        self.rfile.read(length)
        self.send_response(200)
        self.send_header("Content-Type", "audio/mpeg")
        self.send_header("Content-Length", str(len(self.audio)))
        self.end_headers()
        self.wfile.write(self.audio)

    def log_message(self, _format: str, *_arguments: object) -> None:
        return


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: live-kokoro-fixture.py <audio-file> <ready-file>")
    audio_path = Path(sys.argv[1])
    ready_path = Path(sys.argv[2])
    AudioHandler.audio = audio_path.read_bytes()
    if not AudioHandler.audio:
        raise SystemExit("audio fixture is empty")
    server = ThreadingHTTPServer(("127.0.0.1", 0), AudioHandler)
    staged = ready_path.with_suffix(".tmp")
    staged.write_text(f"{server.server_port}\n", encoding="utf-8")
    os.chmod(staged, 0o600)
    staged.replace(ready_path)
    server.serve_forever()


if __name__ == "__main__":
    main()
