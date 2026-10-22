#!/usr/bin/env python3
# A fake Melibea for the smoke (CEL-2-J), stdlib only, run as `python3 -I`.
#
# It listens on one Unix socket that the caller names, and answers the shell's
# protocol just enough to exercise the bubbles worker:
#
# - `subscribe` gets one v1 snapshot of ONE fixture window and the connection
#   stays open until the peer closes it;
# - `restore`, `close` and `minimize` get an `action_result` with status
#   `applied` and nothing else: no change is ever sent, so the window never
#   moves and nothing real is touched.
#
# It refuses a path that exists already (it never deletes a file it did not
# create), a parent directory open to group or others, and anything but a
# caller-named path. The socket is created 0600. SIGTERM stops it and removes
# its socket; that is the only way it ends. It never reads or writes the
# desktop's real Melibea socket: the smoke gives it a path inside a private
# runtime directory.

import json
import os
import signal
import socket
import sys
import threading

MAX_LINE = 64 * 1024
FIXTURE_WINDOW = {"id": 1, "app_id": "fixture.app", "title": "Fixture window", "icon_name": None}


def fail(message):
    print(f"fake-melibea: {message}", file=sys.stderr)
    sys.exit(2)


def envelope(message):
    return (json.dumps({"version": 1, "message": message}) + "\n").encode()


def answer(line):
    """The reply for one request line, or None when there is none."""
    try:
        request = json.loads(line)["request"]
    except (ValueError, KeyError, TypeError):
        return envelope({"type": "error", "code": "invalid_request",
                         "message": "not a request", "supported_versions": []})
    kind = request.get("type") if isinstance(request, dict) else None
    # The smoke reads this log to prove the shell spoke to this fake and not
    # to any other socket.
    print(f"fake-melibea: request {kind or 'invalid'}", flush=True)
    if kind == "subscribe":
        return envelope({"type": "snapshot", "revision": 1, "windows": [FIXTURE_WINDOW]})
    if kind in ("restore", "close", "minimize"):
        window = request.get("window_id")
        return envelope({
            "type": "action_result",
            "operation": kind,
            "requested_id": window,
            "window_id": window if window is not None else FIXTURE_WINDOW["id"],
            "status": "applied",
        })
    return envelope({"type": "error", "code": "invalid_request",
                     "message": "unknown request", "supported_versions": [1]})


def serve(conn, stop):
    conn.settimeout(0.5)
    buffer = b""
    try:
        while not stop.is_set():
            try:
                chunk = conn.recv(4096)
            except socket.timeout:
                continue
            if not chunk:
                return
            buffer += chunk
            if len(buffer) > MAX_LINE:
                return
            while b"\n" in buffer:
                line, buffer = buffer.split(b"\n", 1)
                reply = answer(line)
                if reply is None:
                    continue
                conn.sendall(reply)
                if b'"snapshot"' in reply:
                    # The subscription: keep the connection open, send nothing more.
                    buffer = b""
                    while not stop.is_set():
                        try:
                            if not conn.recv(4096):
                                return
                        except socket.timeout:
                            continue
                        except OSError:
                            return
                    return
    except OSError:
        return
    finally:
        conn.close()


def main(argv):
    if len(argv) != 2 or not os.path.isabs(argv[1]):
        fail("usage: fake-melibea.py ABSOLUTE_SOCKET_PATH")
    path = argv[1]
    parent = os.path.dirname(path)
    if os.path.lexists(path):
        fail(f"{path} exists; refusing to replace it")
    if os.stat(parent).st_mode & 0o077:
        fail(f"{parent} is open to group or others")

    stop = threading.Event()
    signal.signal(signal.SIGTERM, lambda *_: stop.set())
    signal.signal(signal.SIGINT, lambda *_: stop.set())

    previous = os.umask(0o177)
    try:
        server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        server.bind(path)
    finally:
        os.umask(previous)
    os.chmod(path, 0o600)
    server.listen(4)
    server.settimeout(0.2)
    print(f"fake-melibea: listening on {path}", flush=True)

    workers = []
    try:
        while not stop.is_set():
            try:
                conn, _ = server.accept()
            except socket.timeout:
                continue
            worker = threading.Thread(target=serve, args=(conn, stop), daemon=True)
            worker.start()
            workers.append(worker)
            workers = [w for w in workers if w.is_alive()]
    finally:
        stop.set()
        server.close()
        try:
            os.unlink(path)
        except FileNotFoundError:
            pass
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
