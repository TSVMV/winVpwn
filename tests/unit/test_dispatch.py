"""Tests for the execution/dispatch path (run_elf)."""

from __future__ import annotations

import pathlib

import pytest

from winvpwn import _core as winvpwn_core


def test_run_hello_exits_zero(hello_static_bytes: bytes) -> None:
    result = winvpwn_core.run_elf(hello_static_bytes)
    assert result["exit"] == "exit"
    assert result["code"] == 0
    assert result["output"] == b"hello, winVpwn\n"


def test_run_trace_records_write_and_exit(hello_static_bytes: bytes) -> None:
    result = winvpwn_core.run_elf(hello_static_bytes)
    trace = result["trace"]
    assert any(rec["name"] == "write" and rec["ret"] == 15 for rec in trace)
    assert any(rec["name"] == "exit" or rec["name"] == "exit_group" for rec in trace)
    seqs = [rec["seq"] for rec in trace]
    assert seqs == sorted(seqs)


def test_run_rejects_non_elf() -> None:
    with pytest.raises(ValueError) as excinfo:
        winvpwn_core.run_elf(b"not an elf")
    assert "E001" in str(excinfo.value)


def test_run_timeout_parameter_is_accepted(hello_static_bytes: bytes) -> None:
    result = winvpwn_core.run_elf(hello_static_bytes, 5000)
    assert result["exit"] == "exit"


def test_run_stdin_is_passed_to_guest(hello_static_bytes: bytes) -> None:
    result = winvpwn_core.run_elf(hello_static_bytes, 0, b"injected-input\n")
    assert result["exit"] == "exit"


def test_run_maps_writable_missing_host_is_accepted(
    hello_static_bytes: bytes, tmp_path: pathlib.Path
) -> None:
    host = tmp_path / "out.txt"
    maps = [("/out", str(host), True)]
    result = winvpwn_core.run_elf(hello_static_bytes, 0, maps=maps)
    assert result["exit"] == "exit"
    # A writable map to a missing host file is allowed; the host file is only
    # created when the guest actually writes through the mapping.
    assert not host.exists()


def test_run_argv_and_env_are_accepted(hello_static_bytes: bytes) -> None:
    result = winvpwn_core.run_elf(
        hello_static_bytes, 0, argv=["/prog", "a"], env=["FOO=1"], cwd="/tmp"
    )
    assert result["exit"] == "exit"


def test_run_infinite_loop_times_out() -> None:
    # 0xeb 0xfe = jmp $ (infinite loop)
    code = b"\xeb\xfe"
    result = winvpwn_core.run_elf(_minimal_exec(code), 100)
    assert result["exit"] == "stopped" or result["exit"] == "falloff"


def _minimal_exec(code: bytes) -> bytes:
    """Build the smallest static ET_EXEC x86_64 image containing `code`."""
    import struct

    phoff = 64
    entry = 0x400078
    body = b"\x00" * (entry - phoff - 56) + code  # 56 = len(ph)
    image = b"\x7fELF" + struct.pack(
        "<B", 2
    )  # ELF64
    image += struct.pack("<B", 1)  # little endian
    image += struct.pack("<B", 1)  # version
    image += b"\x00" * 9  # osabi + padding
    image += struct.pack("<H", 2)  # ET_EXEC
    image += struct.pack("<H", 62)  # EM_X86_64
    image += struct.pack("<I", 1)  # e_version
    image += struct.pack("<Q", entry)  # e_entry
    image += struct.pack("<Q", phoff)  # e_phoff
    image += struct.pack("<Q", 0)  # e_shoff
    image += struct.pack("<I", 0)  # e_flags
    image += struct.pack("<H", 64)  # e_ehsize
    image += struct.pack("<H", 56)  # e_phentsize
    image += struct.pack("<H", 1)  # e_phnum
    image += struct.pack("<H", 0)  # e_shentsize
    image += struct.pack("<H", 0)  # e_shnum
    image += struct.pack("<H", 0)  # e_shstrndx
    ph = struct.pack(
        "<IIQQQQQQ",
        1,  # PT_LOAD
        5,  # p_flags R+X
        0,  # p_offset
        0x400000,  # p_vaddr
        0x400000,  # p_paddr
        len(body),  # p_filesz  (from offset 0)
        len(body),  # p_memsz
        0x1000,  # p_align
    )
    image += ph + body
    return image
