"""CLI-level tests using the Typer test runner."""

from __future__ import annotations

import pathlib

from typer.testing import CliRunner

from winvpwn.cli import app

runner = CliRunner()


def test_version_command() -> None:
    result = runner.invoke(app, ["version"])
    assert result.exit_code == 0
    assert "winVpwn" in result.stdout


def test_doctor_ok() -> None:
    result = runner.invoke(app, ["doctor"])
    assert result.exit_code == 0
    assert "native engine" in result.stdout
    assert "ok" in result.stdout


def test_doctor_json() -> None:
    result = runner.invoke(app, ["doctor", "--json"])
    assert result.exit_code == 0
    assert "winvpwn_version" in result.stdout


def test_run_missing_file() -> None:
    result = runner.invoke(app, ["run", "/nonexistent/elf"])
    assert result.exit_code == 2


def test_run_hello(hello_static_path: pathlib.Path) -> None:
    result = runner.invoke(app, ["run", str(hello_static_path)])
    assert result.exit_code == 0
    assert "hello, winVpwn" in result.stdout
    assert "exit(0)" in result.stdout


def test_run_hello_json(hello_static_path: pathlib.Path) -> None:
    result = runner.invoke(app, ["run", "--json", str(hello_static_path)])
    assert result.exit_code == 0
    assert '"exit": "exit"' in result.stdout or '"exit": "exit"' in result.stdout


def test_elf_info(hello_static_path: pathlib.Path) -> None:
    result = runner.invoke(app, ["elf", str(hello_static_path)])
    assert result.exit_code == 0
    assert "loadable segments" in result.stdout


def test_asm_disasm_roundtrip() -> None:
    asm = runner.invoke(app, ["asm", "mov eax, 1", "ret"])
    assert asm.exit_code == 0
    hexbytes = asm.stdout.strip()
    dis = runner.invoke(app, ["disasm", hexbytes])
    assert dis.exit_code == 0
    assert "mov" in dis.stdout
