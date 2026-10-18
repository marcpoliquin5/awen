"""``awen-studio``: run the local Studio server (127.0.0.1 by default)."""

from __future__ import annotations

import argparse
import ipaddress
from pathlib import Path
from typing import Optional


def default_static_dir() -> Optional[Path]:
    here = Path(__file__).resolve().parent
    for candidate in (here / "static", here.parent / "frontend" / "dist"):
        if (candidate / "index.html").is_file():
            return candidate
    return None


def main(argv: Optional[list[str]] = None) -> None:
    parser = argparse.ArgumentParser(prog="awen-studio", description=__doc__)
    parser.add_argument("--workspace", default="~/.awen-studio", help="directory holding Studio projects")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--static", type=Path, default=None, help="built frontend (frontend/dist)")
    parser.add_argument("--allow-remote", action="store_true",
                        help="permit a non-loopback --host (Studio has no authentication)")
    args = parser.parse_args(argv)
    try:
        loopback = ipaddress.ip_address(args.host).is_loopback
    except ValueError:
        loopback = args.host == "localhost"
    if not loopback and not args.allow_remote:
        parser.error("Studio has no authentication; bind to 127.0.0.1 or pass --allow-remote deliberately")

    import uvicorn

    from .app import create_app

    static = args.static or default_static_dir()
    app = create_app(Path(args.workspace).expanduser(), static_dir=static)
    print(f"AWEN Studio on http://{args.host}:{args.port}  (workspace {Path(args.workspace).expanduser()};"
          f" frontend {'from ' + str(static) if static else 'not built: API only'})")
    uvicorn.run(app, host=args.host, port=args.port, log_level="info")


if __name__ == "__main__":
    main()
