"""Progress display helpers.

The winVpwn UI spec forbids rotating glyphs, so progress is rendered as static
status lines with an elapsed timer instead of spinners.
"""

from __future__ import annotations

from rich.console import Console, Group
from rich.live import Live
from rich.text import Text


class StaticProgress:
    """A non-animated progress surface.

    Renders a single static status line (updated in place via ``Live``)
    without any spinner or rotating character.
    """

    def __init__(self, console: Console | None = None) -> None:
        self._console = console or Console()
        self._live: Live | None = None

    def __enter__(self) -> StaticProgress:
        self._live = Live(console=self._console, refresh_per_second=4, transient=False)
        self._live.__enter__()
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: object,
    ) -> None:
        if self._live is not None:
            self._live.__exit__(exc_type, exc_val, exc_tb)  # type: ignore[arg-type]
            self._live = None

    def update(self, status: str, detail: str = "") -> None:
        if self._live is None:
            return
        text = Text(status, style="steel_blue")
        if detail:
            text.append(f"  {detail}", style="bright_black")
        group = Group(text)
        self._live.update(group)
