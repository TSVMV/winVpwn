"""Tests for the ELF inspection path (parse_elf)."""

from __future__ import annotations

import pytest

from winvpwn import _core as winvpwn_core


def test_parse_static_exec(hello_static_bytes: bytes) -> None:
    info = winvpwn_core.parse_elf(hello_static_bytes)
    assert info["is_pie"] is False
    assert info["entry"] >= 0x400000
    assert len(info["segments"]) > 0
    for seg in info["segments"]:
        assert seg["memsz"] >= seg["filesz"]


def test_parse_rejects_garbage() -> None:
    with pytest.raises(ValueError) as excinfo:
        winvpwn_core.parse_elf(b"MZ\x90\x00not an elf at all")
    assert "E001" in str(excinfo.value)


def test_parse_rejects_truncated(hello_static_bytes: bytes) -> None:
    with pytest.raises(ValueError) as excinfo:
        winvpwn_core.parse_elf(hello_static_bytes[:64])
    assert str(excinfo.value).startswith("E005") or str(excinfo.value).startswith("E006")


def test_parse_empty_image() -> None:
    with pytest.raises(ValueError) as excinfo:
        winvpwn_core.parse_elf(b"")
    assert "E001" in str(excinfo.value)
