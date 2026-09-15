"""Shared pytest fixtures and import path bootstrap."""

from __future__ import annotations

import pathlib
import sys

import pytest

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "src"))


@pytest.fixture(scope="session")
def hello_static_path() -> pathlib.Path:
    return ROOT / "tests" / "fixtures" / "hello_static"


@pytest.fixture(scope="session")
def hello_static_bytes(hello_static_path: pathlib.Path) -> bytes:
    return hello_static_path.read_bytes()
