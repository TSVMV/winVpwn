"""Command line interface for winVpwn."""

from __future__ import annotations

import sys
from pathlib import Path

import typer
from rich.console import Console

import winvpwn
from winvpwn import _core as core
from winvpwn.doctor import doctor as doctor_cmd
from winvpwn.ui.tables import base_table
from winvpwn.ui.theme import ERROR, OK, PRIMARY, WARN

app = typer.Typer(
    name="winvpwn",
    help="Run Linux ELF binaries on Windows without WSL, VMs or Docker.",
    no_args_is_help=True,
)

_console = Console(highlight=False)


def _make_console(json_mode: bool = False, no_color: bool = False, quiet: bool = False) -> Console:
    if no_color:
        return Console(highlight=False, color_system=None)
    return _console


@app.command()
def doctor(
    json: bool = typer.Option(False, "--json", help="Emit machine-readable JSON."),
    no_color: bool = typer.Option(False, "--no-color", help="Disable ANSI colors."),
) -> None:
    """Check the local environment for winVpwn requirements."""
    console = _make_console(no_color=no_color)
    raise typer.Exit(code=doctor_cmd(console, json_out=json))


@app.command()
def run(
    elf: Path = typer.Argument(..., help="Path to a static ET_EXEC x86_64 ELF."),
    timeout: float = typer.Option(0.0, "--timeout", help="Execution timeout in milliseconds."),
    stdin_file: Path | None = typer.Option(
        None, "--stdin", help="Feed this file to guest stdin (fd 0)."
    ),
    map_file: list[str] = typer.Option(
        [],
        "--map",
        help="Map a host file into the guest VFS as guest=host or guest=host:rw.",
    ),
    arg: list[str] = typer.Option([], "--arg", help="Guest argv entry (repeatable)."),
    env: list[str] = typer.Option([], "--env", help="Guest env entry KEY=VAL (repeatable)."),
    cwd: str = typer.Option("/", "--cwd", help="Guest working directory."),
    json: bool = typer.Option(False, "--json", help="Emit machine-readable JSON."),
    no_color: bool = typer.Option(False, "--no-color", help="Disable ANSI colors."),
    quiet: bool = typer.Option(False, "--quiet", help="Suppress non-essential output."),
) -> None:
    """Load and execute an ELF binary in the emulator."""
    console = _make_console(no_color=no_color)
    try:
        image = elf.read_bytes()
        stdin_bytes = stdin_file.read_bytes() if stdin_file is not None else None
        argv = arg if arg else [str(elf)]
        maps = [_parse_map(item) for item in map_file]
        result = core.run_elf(
            image,
            int(timeout),
            stdin_bytes,
            argv,
            env,
            maps,
            cwd,
        )
    except (ValueError, OSError) as exc:
        console.print(f"[{ERROR}]error[/] {exc}")
        raise typer.Exit(code=2) from exc

    output = result["output"]
    if output:
        sys.stdout.buffer.write(output)
        sys.stdout.buffer.flush()

    if json:
        import json as _json

        payload = dict(result)
        payload["output"] = _json.dumps(payload["output"].decode("utf-8", "replace"))
        console.print_json(data=payload)
        raise typer.Exit(code=_exit_code(result))

    if not quiet:
        console.print()
        _print_run_summary(console, result)
        _print_trace(console, result.get("trace", []))
    raise typer.Exit(code=_exit_code(result))


def _parse_map(spec: str) -> tuple[str, str, bool]:
    """Parse `guest=host` or `guest=host:rw`."""
    if "=" not in spec:
        raise ValueError(f"invalid --map {spec!r}: expected guest=host")
    guest, rest = spec.split("=", 1)
    writable = False
    host = rest
    if rest.endswith(":rw"):
        writable = True
        host = rest[: -len(":rw")]
    elif rest.endswith(":ro"):
        host = rest[: -len(":ro")]
    guest = guest.strip()
    host = host.strip()
    if not guest.startswith("/"):
        guest = "/" + guest
    if not guest or not host:
        raise ValueError(f"invalid --map {spec!r}")
    return guest, host, writable


def _exit_code(result: dict[str, object]) -> int:
    exit_kind = result.get("exit")
    if exit_kind == "exit":
        code = result.get("code")
        return int(code) if isinstance(code, int) else 0
    return 1


def _print_run_summary(console: Console, result: dict[str, object]) -> None:
    exit_kind = result["exit"]
    if exit_kind == "exit":
        code = result.get("code", 0)
        code_str = str(code) if code is not None else "0"
        style = OK if code in (0, "0", None) else WARN
        console.print(f"[{style}]exit({code_str})[/]")
    elif exit_kind == "falloff":
        rip = result.get("rip", 0)
        rip_str = str(rip) if rip is not None else "0"
        console.print(f"[{WARN}]instruction falloff at {rip_str}[/]")
    else:
        console.print(f"[{WARN}]stopped by user[/]")


def _print_trace(console: Console, trace: list[dict[str, object]]) -> None:
    if not trace:
        console.print("[bright_black]no syscalls were executed[/]")
        return
    table = base_table(title="syscall trace")
    table.add_column("#")
    table.add_column("syscall")
    table.add_column("args")
    table.add_column("ret")
    for rec in trace:
        args_raw = rec.get("args")
        args_list = args_raw if isinstance(args_raw, list) else []
        args = ", ".join(str(a) for a in args_list)
        table.add_row(str(rec.get("seq", "")), str(rec.get("name", "")), args, str(rec.get("ret", "")))
    console.print(table)


@app.command()
def elf(
    elf: Path = typer.Argument(..., help="Path to an ELF image."),
    json: bool = typer.Option(False, "--json", help="Emit machine-readable JSON."),
    no_color: bool = typer.Option(False, "--no-color", help="Disable ANSI colors."),
) -> None:
    """Inspect a loadable ELF image without executing it."""
    console = _make_console(no_color=no_color)
    try:
        image = elf.read_bytes()
        info = core.parse_elf(image)
    except (ValueError, OSError) as exc:
        console.print(f"[{ERROR}]error[/] {exc}")
        raise typer.Exit(code=2) from exc

    if json:
        console.print_json(data=info)
        raise typer.Exit(code=0)

    console.print(f"[{PRIMARY}]entry[/] 0x{info['entry']:x}")
    console.print(f"[{PRIMARY}]pie[/] {info['is_pie']}")
    table = base_table(title="loadable segments")
    table.add_column("vaddr")
    table.add_column("memsz")
    table.add_column("filesz")
    table.add_column("flags")
    table.add_column("align")
    for seg in info["segments"]:
        flags = f"{seg['flags']:08x}"
        table.add_row(
            f"0x{seg['vaddr']:x}",
            f"0x{seg['memsz']:x}",
            f"0x{seg['filesz']:x}",
            flags,
            f"0x{seg['align']:x}",
        )
    console.print(table)


@app.command()
def asm(
    code: list[str] = typer.Argument(..., help="Assembly source lines."),
    address: int = typer.Option(0x400000, "--address", help="Assemble at this address."),
    json: bool = typer.Option(False, "--json", help="Emit machine-readable JSON."),
    no_color: bool = typer.Option(False, "--no-color", help="Disable ANSI colors."),
) -> None:
    """Assemble x86-64 source text into a hex byte string."""
    console = _make_console(no_color=no_color)
    from winvpwn.toolchain import assemble

    source = "\n".join(code)
    try:
        raw = assemble(source, address)
    except (ValueError, ImportError) as exc:
        console.print(f"[{ERROR}]error[/] {exc}")
        raise typer.Exit(code=2) from exc
    hexdump = raw.hex()
    if json:
        console.print_json(data={"hex": hexdump, "size": len(raw)})
        raise typer.Exit(code=0)
    console.print(hexdump)
    raise typer.Exit(code=0)


@app.command()
def disasm(
    hexbytes: str = typer.Argument(..., help="Hex-encoded byte string to disassemble."),
    address: int = typer.Option(0x400000, "--address", help="Start address."),
    json: bool = typer.Option(False, "--json", help="Emit machine-readable JSON."),
    no_color: bool = typer.Option(False, "--no-color", help="Disable ANSI colors."),
) -> None:
    """Disassemble a hex-encoded byte string."""
    console = _make_console(no_color=no_color)
    from winvpwn.toolchain import disassemble

    try:
        raw = bytes.fromhex(hexbytes)
        instructions = disassemble(raw, address)
    except (ValueError, ImportError) as exc:
        console.print(f"[{ERROR}]error[/] {exc}")
        raise typer.Exit(code=2) from exc

    if json:
        console.print_json(data=[i.__dict__ for i in instructions])
        raise typer.Exit(code=0)
    table = base_table(title="instructions")
    table.add_column("address")
    table.add_column("bytes")
    table.add_column("mnemonic")
    table.add_column("operands")
    for insn in instructions:
        table.add_row(f"0x{insn.address:x}", "", insn.mnemonic, insn.op_str)
    console.print(table)


@app.command()
def version() -> None:
    """Print the winVpwn version."""
    _console.print(f"winVpwn {winvpwn.__version__}")


if __name__ == "__main__":
    app()
