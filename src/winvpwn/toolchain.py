"""Assembly and disassembly helpers wrapping capstone/keystone."""

from __future__ import annotations

from dataclasses import dataclass

import capstone
import keystone


@dataclass(frozen=True)
class Instruction:
    address: int
    size: int
    mnemonic: str
    op_str: str


def assemble(code: str, address: int = 0x400000) -> bytes:
    """Assemble x86-64 source text into raw bytes."""
    ks = keystone.Ks(keystone.KS_ARCH_X86, keystone.KS_MODE_64)
    encoding, _count = ks.asm(code, address)
    if encoding is None:
        raise ValueError("assembly produced no output")
    return bytes(encoding)


def disassemble(code: bytes, address: int = 0x400000) -> list[Instruction]:
    """Disassemble raw x86-64 bytes into an instruction list."""
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
    md.detail = False
    result: list[Instruction] = []
    for insn in md.disasm(code, address):
        result.append(
            Instruction(
                address=insn.address,
                size=insn.size,
                mnemonic=insn.mnemonic,
                op_str=insn.op_str,
            )
        )
    return result
