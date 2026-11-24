"""Small bounded-retention helpers for Voicebot's local logs and prompt snapshots."""
import json
import os

try:
    import fcntl
except ImportError:  # Windows uses msvcrt for the same sidecar lock.
    fcntl = None
    import msvcrt


def append_bounded(path: str, data: bytes, max_bytes: int, *, preserve_lines: bool = False,
                   trim_to: int | None = None) -> None:
    """Append bytes while keeping the file at or below ``max_bytes``.

    When a trim is needed, retain the newest part of the file and then append. By default the
    trim leaves 20% headroom so streaming writers do not rewrite the entire log for every chunk.
    """
    if max_bytes <= 0:
        return
    if len(data) > max_bytes:
        data = data[-max_bytes:]
    parent = os.path.dirname(path)
    if parent:
        os.makedirs(parent, exist_ok=True)
    try:
        size = os.path.getsize(path)
    except OSError:
        size = 0
    if size + len(data) <= max_bytes:
        with open(path, "ab") as f:
            f.write(data)
        return

    target_size = max_bytes * 4 // 5 if trim_to is None else trim_to
    target_size = max(0, min(target_size, max_bytes))
    keep = max(0, target_size - len(data))
    tail = b""
    if size and keep:
        with open(path, "rb") as f:
            start = max(0, size - keep)
            f.seek(start)
            if preserve_lines and start:
                f.readline()  # discard the partial oldest line
            tail = f.read()

    temp_path = path + ".tmp"
    with open(temp_path, "wb") as f:
        f.write(tail)
        f.write(data)
    os.replace(temp_path, path)


def append_bounded_jsonl(path: str, event: dict, max_bytes: int) -> None:
    """Append one JSONL event, omitting oversized payloads rather than writing invalid JSON."""
    parent = os.path.dirname(path)
    if parent:
        os.makedirs(parent, exist_ok=True)
    with open(path + ".lock", "a+b") as lock_file:
        if fcntl is not None:
            fcntl.flock(lock_file.fileno(), fcntl.LOCK_EX)
        else:
            lock_file.seek(0)
            msvcrt.locking(lock_file.fileno(), msvcrt.LK_LOCK, 1)
        try:
            line = json.dumps(event, default=str, separators=(",", ":")).encode("utf-8")
            if len(line) + 1 > max_bytes:
                line = json.dumps({"t": event.get("t"), "kind": event.get("kind", "event"),
                                   "truncated": True}, separators=(",", ":")).encode("utf-8")
            if len(line) + 1 > max_bytes:
                line = b'{"truncated":true}'
            record = line + b"\n"
            try:
                with open(path, "rb") as f:
                    f.seek(0, os.SEEK_END)
                    if f.tell():
                        f.seek(-1, os.SEEK_END)
                        if f.read(1) != b"\n":
                            record = b"\n" + record
            except FileNotFoundError:
                pass
            append_bounded(path, record, max_bytes, preserve_lines=True)
        finally:
            if fcntl is not None:
                fcntl.flock(lock_file.fileno(), fcntl.LOCK_UN)
            else:
                lock_file.seek(0)
                msvcrt.locking(lock_file.fileno(), msvcrt.LK_UNLCK, 1)


def prune_prompt_history(directory: str, keep: int = 20) -> int:
    """Keep only the newest ``keep`` system-prompt backups; return the removed count."""
    if keep < 0:
        raise ValueError("keep must not be negative")
    try:
        names = sorted(name for name in os.listdir(directory)
                       if name.startswith("system_prompt-") and name.endswith(".md")
                       and os.path.isfile(os.path.join(directory, name)))
    except FileNotFoundError:
        return 0
    removed = 0
    for name in names[:-keep] if keep else names:
        try:
            os.remove(os.path.join(directory, name))
            removed += 1
        except FileNotFoundError:
            pass
    return removed
