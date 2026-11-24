"""Small helper for reporting a server's actual OS-assigned listening port."""
import os


def report_bound_port(server, path: str) -> int:
    """Atomically write the first bound Uvicorn socket's port to ``path``."""
    if not getattr(server, "started", False):
        raise RuntimeError("cannot report a port before the server has started")

    sockets = [sock for listener in (getattr(server, "servers", None) or [])
               for sock in (getattr(listener, "sockets", None) or [])]
    if not sockets:
        raise RuntimeError("server started without an accessible listening socket")

    port = int(sockets[0].getsockname()[1])
    if not 1 <= port <= 65535:
        raise RuntimeError(f"server reported invalid listening port {port}")

    parent = os.path.dirname(os.path.abspath(path))
    os.makedirs(parent, exist_ok=True)
    temporary = f"{path}.{os.getpid()}.tmp"
    with open(temporary, "w", encoding="ascii") as output:
        output.write(f"{port}\n")
        output.flush()
        os.fsync(output.fileno())
    os.replace(temporary, path)
    return port
