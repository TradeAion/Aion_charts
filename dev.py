#!/usr/bin/env python3
"""
Aion_charts dev server — watch + build + serve in one command.

Usage:
    python dev.py              # build once, then serve + watch
    python dev.py --no-watch   # build once, then serve (no file watching)
    python dev.py --serve-only # skip initial build, just serve + watch

Watches all .rs files under src/ and wasm/src/. When a change is detected,
automatically runs wasm-pack build and the browser just needs a refresh
(serve.py already sends no-cache headers).
"""

import http.server
import os
import subprocess
import sys
import threading
import time

# ─── Configuration ────────────────────────────────────────────────────────────

PORT = 8080
WATCH_DIRS = ["src", os.path.join("wasm", "src")]
WATCH_EXTENSIONS = {".rs", ".toml"}
DEBOUNCE_SECONDS = 0.5
BUILD_CMD = ["wasm-pack", "build", "--target", "web", "wasm"]

# ─── Helpers ──────────────────────────────────────────────────────────────────

ROOT = os.path.dirname(os.path.abspath(__file__))
os.chdir(ROOT)


def log(msg):
    print(f"\033[36m[dev]\033[0m {msg}", flush=True)


def log_ok(msg):
    print(f"\033[32m[dev]\033[0m {msg}", flush=True)


def log_err(msg):
    print(f"\033[31m[dev]\033[0m {msg}", flush=True)


def run_build():
    """Run wasm-pack build. Returns True on success."""
    log("Building WASM...")
    t0 = time.time()
    result = subprocess.run(BUILD_CMD, capture_output=False)
    elapsed = time.time() - t0
    if result.returncode == 0:
        log_ok(f"Build succeeded in {elapsed:.1f}s — refresh browser to see changes")
        return True
    else:
        log_err(f"Build FAILED (exit code {result.returncode})")
        return False


# ─── File watcher (pure stdlib, no dependencies) ─────────────────────────────

def collect_mtimes(dirs, extensions):
    """Scan dirs and return {filepath: mtime} for matching extensions."""
    result = {}
    for d in dirs:
        full = os.path.join(ROOT, d)
        if not os.path.isdir(full):
            continue
        for dirpath, _, filenames in os.walk(full):
            for fname in filenames:
                if os.path.splitext(fname)[1] in extensions:
                    fpath = os.path.join(dirpath, fname)
                    try:
                        result[fpath] = os.path.getmtime(fpath)
                    except OSError:
                        pass
    return result


def watch_loop():
    """Poll for file changes and trigger rebuilds."""
    log(f"Watching {', '.join(WATCH_DIRS)} for changes...")
    prev = collect_mtimes(WATCH_DIRS, WATCH_EXTENSIONS)

    while True:
        time.sleep(DEBOUNCE_SECONDS)
        curr = collect_mtimes(WATCH_DIRS, WATCH_EXTENSIONS)

        changed = []
        for fpath, mtime in curr.items():
            if fpath not in prev or prev[fpath] != mtime:
                changed.append(os.path.relpath(fpath, ROOT))
        for fpath in prev:
            if fpath not in curr:
                changed.append(os.path.relpath(fpath, ROOT) + " (deleted)")

        if changed:
            # Show which files changed (max 5)
            shown = changed[:5]
            extra = f" (+{len(changed) - 5} more)" if len(changed) > 5 else ""
            log(f"Changed: {', '.join(shown)}{extra}")
            run_build()
            prev = collect_mtimes(WATCH_DIRS, WATCH_EXTENSIONS)
        else:
            prev = curr


# ─── HTTP server (same as serve.py) ──────────────────────────────────────────

class NoCacheHandler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header(
            "Cache-Control", "no-store, no-cache, must-revalidate, max-age=0"
        )
        self.send_header("Pragma", "no-cache")
        self.send_header("Expires", "0")
        super().end_headers()

    def guess_type(self, path):
        if str(path).endswith(".wasm"):
            return "application/wasm"
        return super().guess_type(path)

    # Suppress per-request log spam — only show errors
    def log_message(self, format, *args):
        if args and str(args[0]).startswith(("4", "5")):
            super().log_message(format, *args)


class DevServer(http.server.ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True

    def handle_error(self, request, client_address):
        exc_type, exc, _ = sys.exc_info()
        if exc_type in (BrokenPipeError, ConnectionAbortedError, ConnectionResetError):
            log(f"Client disconnected while serving {client_address}: {exc}")
            return
        super().handle_error(request, client_address)


def start_server():
    server = DevServer(("", PORT), NoCacheHandler)
    log_ok(f"Serving at http://localhost:{PORT}/demo/")
    server.serve_forever()


# ─── Main ─────────────────────────────────────────────────────────────────────

def main():
    no_watch = "--no-watch" in sys.argv
    serve_only = "--serve-only" in sys.argv

    # Initial build (unless --serve-only)
    if not serve_only:
        if not run_build():
            log_err("Initial build failed — starting server anyway so you can fix errors")

    # Start file watcher in background thread
    if not no_watch:
        watcher = threading.Thread(target=watch_loop, daemon=True)
        watcher.start()

    # Start HTTP server (blocks)
    start_server()


if __name__ == "__main__":
    main()
