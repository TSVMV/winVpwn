#!/usr/bin/env python3
"""Minimal winVpwn example: run a static ELF and print its captured output."""

from __future__ import annotations

import sys
from pathlib import Path

from winvpwn import _core as winvpwn_core

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <static-elf>", file=sys.stderr)
        raise SystemExit(2)
    image = Path(sys.argv[1]).read_bytes()
    result = winvpwn_core.run_elf(image)
    sys.stdout.buffer.write(result["output"])
    sys.stdout.buffer.flush()
    sys.exit(result.get("code", 1))
