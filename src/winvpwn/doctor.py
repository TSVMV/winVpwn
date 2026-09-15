"""Environment diagnostics for the winVpwn CLI."""

from __future__ import annotations

import platform
import sys

from rich.console import Console
from rich.table import Table
from rich.text import Text

import winvpwn
from winvpwn.ui.theme import DIM, ERROR, OK, PRIMARY, WARN


def _check_engine() -> tuple[str, str]:
    """Return (status, detail) for the native engine availability."""
    try:
        from winvpwn import _core as winvpwn_core  # noqa: F401

        return "ok", "native extension loadable"
    except Exception as exc:  # pragma: no cover - defensive
        return "error", f"cannot import winvpwn._core: {exc}"


def _check_python() -> tuple[str, str]:
    version = platform.python_version()
    if sys.version_info >= (3, 10):  # noqa: UP036
        return "ok", version
    return "error", f"Python {version} (< 3.10 unsupported)"


def _check_os() -> tuple[str, str]:
    system = platform.system()
    arch = platform.machine()
    return "ok", f"{system} / {arch}"


def _check_toolchain() -> tuple[str, str]:
    missing = []
    for name in ("capstone", "keystone"):
        try:
            __import__(name)
        except ImportError:
            missing.append(name)
    if missing:
        return "warn", f"missing optional modules: {', '.join(missing)}"
    return "ok", "capstone, keystone available"


def doctor(console: Console, json_out: bool = False) -> int:
    """Run environment checks and print a report."""
    checks = {
        "winvpwn version": ("ok", winvpwn.__version__),
        "native engine": _check_engine(),
        "python": _check_python(),
        "platform": _check_os(),
        "toolchain": _check_toolchain(),
    }
    if json_out:

        console.print_json(
            data={"winvpwn_version": winvpwn.__version__, "checks": {k: v for k, v in checks.items()}}
        )
        return 0

    table = Table(title="winvpwn doctor", header_style=PRIMARY, border_style=DIM)
    table.add_column("Check", style=PRIMARY)
    table.add_column("Status")
    table.add_column("Detail")
    exit_code = 0
    for name, (status, detail) in checks.items():
        style = {"ok": OK, "warn": WARN, "error": ERROR}[status]
        if status == "error":
            exit_code = 1
        table.add_row(name, Text(status, style=style), detail)
    table.add_section()
    table.add_row("Overall", "ok" if exit_code == 0 else "failed", "", end_section=True)
    console.print(table)
    return exit_code
