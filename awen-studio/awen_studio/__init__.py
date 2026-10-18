"""AWEN Studio: local web app over the real AWEN backends (simulation only)."""

from .app import create_app
from .workspace import Workspace

__all__ = ["Workspace", "create_app"]
__version__ = "0.1.0"
