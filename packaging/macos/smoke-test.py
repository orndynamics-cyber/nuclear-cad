#!/usr/bin/env python3
"""Launch a built Nuclear.app on macOS and drive it over the control channel.

Usage: smoke-test.py <path/to/Nuclear.app> <out-dir>

Starts the app with the sample design, waits for the control channel, asks for the open
documents, a screenshot of the real window and a GPU render of the model, then quits it. Fails
when the app exits early, never answers, or a picture comes back empty.
"""
import json
import os
import secrets
import socket
import subprocess
import sys
import time

PORT = 27900


def main() -> int:
    app, out = sys.argv[1], os.path.abspath(sys.argv[2])
    os.makedirs(out, exist_ok=True)
    token = secrets.token_hex(32)
    env = dict(os.environ, SOLIDCRAFT_CONTROL_TOKEN=token)
    log = open(os.path.join(out, "app.log"), "w")
    proc = subprocess.Popen(
        [os.path.join(app, "Contents/MacOS/Nuclear"), "--sample", "--control", str(PORT)],
        env=env, stdout=log, stderr=subprocess.STDOUT)

    sock = None
    deadline = time.time() + 180
    while time.time() < deadline:
        if proc.poll() is not None:
            print(f"the app exited early with code {proc.returncode}", file=sys.stderr)
            return 1
        try:
            sock = socket.create_connection(("127.0.0.1", PORT), timeout=5)
            break
        except OSError:
            time.sleep(1)
    if sock is None:
        print("the control channel never opened", file=sys.stderr)
        proc.kill()
        return 1
    print(f"control channel open after {180 - (deadline - time.time()):.0f}s")
    sock.settimeout(120)
    reader = sock.makefile("r")
    next_id = [0]

    def call(method, params=None):
        next_id[0] += 1
        req = {"id": next_id[0], "method": method, "params": params or {}}
        if next_id[0] == 1:
            req["token"] = token
        sock.sendall((json.dumps(req) + "\n").encode())
        while True:
            line = reader.readline()
            if not line:
                raise RuntimeError(f"connection closed during {method}")
            reply = json.loads(line)
            if reply.get("id") == req["id"]:
                print(f"{method}: {json.dumps(reply)[:400]}")
                return reply

    ok = True
    ok &= call("ui.documents").get("ok", False)
    # Let a few frames present before taking the window's picture.
    time.sleep(5)
    for method, name, extra in (
        ("ui.screenshot", "window.png", {}),
        ("ui.render", "render.png", {"width": 1280, "height": 800}),
    ):
        path = os.path.join(out, name)
        reply = call(method, dict(extra, path=path, allow_paths=[out]))
        size = os.path.getsize(path) if os.path.exists(path) else 0
        print(f"{name}: {size} bytes")
        ok &= bool(reply.get("ok")) and size > 1000
    try:
        call("app.quit", {"force": True})
    except Exception as e:  # the window may close before the reply arrives
        print(f"app.quit: {e}")
    try:
        proc.wait(timeout=30)
    except subprocess.TimeoutExpired:
        proc.kill()
    print("PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
