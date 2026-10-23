"""Zero-seed generator for the OpenSteamTool ``ipc`` channel.

Why this channel exists
-----------------------
``pattern`` only tells the DLL *where* a function lives. The IPC layer needs more:
which interface vtable a call belongs to, and — crucially — the per-method
``funcHash`` / ``fencepost`` values that Steam puts **on the wire**. The DLL
matches incoming IPC calls by ``(interfaceID, funcHash)``, so without these
values every IPC handler stays disabled and nothing gets authorised.

How the values are recovered without any prior data file
--------------------------------------------------------
Each client-side IPC wrapper function in ``steamclient64.dll``:

* references the interface-name literal (``"IClientUser"``, shared, many refs),
* references its own method-name literal (``"GetSteamID"``, referenced by
  exactly one function — the wrapper),
* embeds the two wire constants as immediates written into the request header::

      c7 45 40 00 32 fc d6      mov dword ptr [rbp+40h], 0D6FC3200h  ; funcHash
      c7 45 40 a5 8c 05 d7      mov dword ptr [rbp+40h], 0D7058CA5h  ; fencepost

So: method name -> unique referencing function -> two immediates -> (funcHash,
fencepost). ``argc`` is the number of ``in`` parameters from our own IDL, and
``interface_id`` comes from the IDL enum. ``wrapper_rva`` is the function itself.

The IDL is the same ``IPCMessages.steamd`` the DLL is built from, so the method
list and arity are ours; only the two wire constants come from Steam.
"""

from __future__ import annotations

import re
import struct
from dataclasses import dataclass, field

from . import pe as pe_mod


class IpcError(RuntimeError):
    pass


@dataclass
class IpcMethod:
    name: str
    argc: int
    func_hash: int = 0
    fencepost: int = 0
    wrapper_rva: int | None = None


@dataclass
class IpcInterface:
    name: str
    interface_id: int
    methods: list[IpcMethod] = field(default_factory=list)


# --------------------------------------------------------------------------- IDL

_IDL_INTERFACE_RE = re.compile(r"interface\s+([A-Za-z_]\w*)\s*\{")
_IDL_ENUM_RE = re.compile(r"enum\s+EIPCInterface\s*:\s*\w+\s*\{(.*?)\};", re.S)
_IDL_METHOD_RE = re.compile(
    r"([A-Za-z_][\w:<>]*)\s+([A-Za-z_]\w*)\s*\(([^;]*?)\)\s*;", re.S
)


def _matching_brace(text: str, open_index: int) -> int:
    depth = 0
    for i in range(open_index, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return i
    raise IpcError("unbalanced braces in IDL")


def parse_idl(text: str) -> list[IpcInterface]:
    """Extract interface ids and their method declarations from the IDL."""
    ids: dict[str, int] = {}
    enum = _IDL_ENUM_RE.search(text)
    if enum:
        for line in enum.group(1).splitlines():
            line = line.strip().rstrip(",;")
            if not line or "=" not in line:
                continue
            name, value = line.split("=", 1)
            name = name.strip()
            try:
                ids[name] = int(value.strip().rstrip(",;"), 0)
            except ValueError:
                continue

    out: list[IpcInterface] = []
    for match in _IDL_INTERFACE_RE.finditer(text):
        name = match.group(1)
        body_start = text.index("{", match.start())
        body = text[body_start + 1 : _matching_brace(text, body_start)]
        iface = IpcInterface(name=name, interface_id=ids.get(name, 0))
        for decl in _IDL_METHOD_RE.finditer(body):
            mname, params = decl.group(2), decl.group(3)
            argc = sum(1 for p in params.split(",") if re.match(r"\s*in\b", p))
            iface.methods.append(IpcMethod(name=mname, argc=argc))
        if iface.methods:
            out.append(iface)
    return out


# --------------------------------------------------------------------- extraction

_MOV_DWORD_PATTERNS = (
    (0xC7, 0x45, 7),           # mov dword ptr [rbp+disp8], imm32
    (0xC7, 0x44, 0x24, 8),     # mov dword ptr [rsp+disp8], imm32
    (0xC7, 0x84, 0x24, 11),    # mov dword ptr [rsp+disp32], imm32
)


def wire_constants(image: pe_mod.PE, body: bytes) -> list[tuple[int, int]]:
    """``[(offset, immediate)]`` for dword stack writes inside a wrapper."""
    found: list[tuple[int, int]] = []
    for i in range(len(body) - 3):
        if body[i] != 0xC7:
            continue
        for pattern in _MOV_DWORD_PATTERNS:
            prefix, length = pattern[:-1], pattern[-1]
            if i + length > len(body):
                continue
            if body[i : i + len(prefix)] != bytes(prefix):
                continue
            (value,) = struct.unpack_from("<I", body, i + length - 4)
            if value:
                found.append((i, value))
            break
    return found


def find_wrapper(
    image: pe_mod.PE,
    iface_name: str,
    method_name: str,
    method_names: set[str],
) -> int:
    """The unique function that references this method's name literal.

    Two candidates usually show up: the real wrapper and a shared dispatcher
    that mentions every method name (it references *one* method literal only,
    the dispatcher references many). We keep the one that names exactly one
    declared method.
    """
    cands: set[int] = set()
    for rva in image.find_cstring(method_name):
        cands.update(image.funcs_referencing(rva))
    if len(cands) > 1:
        narrowed = set()
        for fn_rva in cands:
            fn = image.func_at(fn_rva)
            if fn is None:
                continue
            texts = set(image.strings_in_func(fn[0], fn[1], 4).values())
            if iface_name in texts and len(texts & method_names) == 1:
                narrowed.add(fn_rva)
        cands = narrowed
    if len(cands) != 1:
        raise IpcError(
            f"{iface_name}::{method_name}: {len(cands)} wrapper candidates "
            f"({', '.join(f'0x{c:X}' for c in sorted(cands))})"
        )
    return next(iter(cands))


def extract(
    image: pe_mod.PE,
    iface: IpcInterface,
    method: IpcMethod,
    method_names: set[str],
) -> None:
    wrapper = find_wrapper(image, iface.name, method.name, method_names)
    fn = image.func_at(wrapper)
    if fn is None:
        raise IpcError(f"{iface.name}::{method.name}: wrapper 0x{wrapper:X} has no unwind info")
    body = image.read(fn[0], fn[1] - fn[0])
    constants = wire_constants(image, body)
    if len(constants) < 2:
        raise IpcError(
            f"{iface.name}::{method.name}: expected funcHash+fencepost immediates, "
            f"found {len(constants)}"
        )
    method.wrapper_rva = wrapper
    method.func_hash = constants[0][1]
    method.fencepost = constants[1][1]


def build(image: pe_mod.PE, idl_text: str) -> tuple[list[IpcInterface], list[str]]:
    """Resolve every IDL method against the image; return kept data + failures."""
    interfaces = parse_idl(idl_text)
    method_names = {m.name for iface in interfaces for m in iface.methods}
    failures: list[str] = []
    for iface in interfaces:
        kept: list[IpcMethod] = []
        for method in iface.methods:
            try:
                extract(image, iface, method, method_names)
            except IpcError as exc:
                failures.append(str(exc))
                continue
            kept.append(method)
        iface.methods = kept
    return [i for i in interfaces if i.methods], failures


# ------------------------------------------------------------------------ emit

HEADER = (
    "# MACHINE-GENERATED by tools/pattern-tracker (ipc channel) - do not hand-edit\n"
    "# wire constants (funcHash/fencepost) extracted from steamclient64.dll wrappers\n"
)


def dump(interfaces: list[IpcInterface]) -> str:
    lines = [HEADER]
    for iface in sorted(interfaces, key=lambda i: i.name):
        lines.append(f"[{iface.name}]")
        lines.append(f"interface_id = {iface.interface_id}")
        lines.append("")
        for method in sorted(iface.methods, key=lambda m: m.name):
            lines.append(f"[{iface.name}.{method.name}]")
            lines.append(f'funcHash = "0x{method.func_hash:08X}"')
            lines.append(f'wrapper_rva = "0x{method.wrapper_rva:X}"')
            lines.append(f'fencepost = "0x{method.fencepost:08X}"')
            lines.append(f"argc = {method.argc}")
            lines.append("")
    return "\n".join(lines)
