"""Table helpers for the CLI output."""

from __future__ import annotations

from rich import box
from rich.table import Table

from winvpwn.ui.theme import DIM, PRIMARY


def base_table(title: str | None = None) -> Table:
    """A table following the winVpwn rounded style."""
    return Table(
        title=title,
        show_header=True,
        header_style=PRIMARY,
        row_styles=["", "dim"],
        border_style=DIM,
        padding=(0, 1),
        box=box.ROUNDED,
    )
