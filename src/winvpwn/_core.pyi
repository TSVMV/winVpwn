# Type stubs for the winvpwn._core native extension (maturin / PyO3).
from __future__ import annotations

from typing import Any

# run_elf(image: bytes, timeout_ms: int = 0) -> dict:
#   {
#     "exit": str,                      # "exit" | "falloff" | "stopped"
#     "code": int,                      # present when exit == "exit"
#     "rip": int,                       # present when exit == "falloff"
#     "output": bytes,
#     "trace": list[dict[str, Any]],    # {seq, nr, name, args, ret, rip}
#   }
def run_elf(image: bytes, timeout_ms: int = 0) -> dict[str, Any]: ...

# parse_elf(image: bytes) -> dict:
#   {
#     "entry": int,
#     "is_pie": bool,
#     "segments": list[dict[str, Any]],  # {vaddr, memsz, filesz, flags, align}
#   }
def parse_elf(image: bytes) -> dict[str, Any]: ...
