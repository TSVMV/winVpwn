"""Shared UI building blocks.

The color palette and table style follow the winVpwn UI spec:
low-saturation colors, rounded tables, trees, and progress indicators that
never use rotating glyphs.
"""

from __future__ import annotations

from typing import Final

# Low-saturation palette
PRIMARY: Final[str] = "cyan"
SECONDARY: Final[str] = "dodger_blue"
OK: Final[str] = "green"
WARN: Final[str] = "gold3"
ERROR: Final[str] = "indian_red"
DIM: Final[str] = "bright_black"
ACCENT: Final[str] = "steel_blue"
