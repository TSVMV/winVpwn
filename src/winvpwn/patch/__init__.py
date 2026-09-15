"""Binary patching helpers (stage 1: placeholder API).

The patching subsystem is planned for a later stage. This module defines the
public surface so imports do not break, and raises a clear error until
implemented.
"""

from __future__ import annotations

from winvpwn import __version__  # noqa: F401

__all__ = ["__version__", "patch_file"]


def patch_file(*_args: object, **_kwargs: object) -> None:
    """Apply a patch to an ELF image (not implemented in stage 1)."""
    raise NotImplementedError("patching is planned for a later stage of winVpwn")
