"""Command line entry point for the pattern tracker.

Subcommands
-----------
``survey <dll> <truth.toml>``
    Print literals / call-graph context around each known hook (bootstrapping a
    new rule).
``resolve <dll> --rules rules/<channel>.toml [--out-dir pattern]``
    Emit ``<sha256>.toml`` for every hook that resolves; report the rest.
``verify <dll> <truth.toml> --rules ...``
    Diff the resolver against a ground-truth TOML (regression gate).
"""

from __future__ import annotations

import argparse
import hashlib
import sys
from pathlib import Path

from . import ipc as ipc_mod
from . import layout as layout_mod
from . import pe as pe_mod
from . import resolve as resolve_mod
from . import sig as sig_mod
from . import tomlio

ROOT = Path(__file__).resolve().parents[1]


def sha256_of(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def cmd_resolve(args: argparse.Namespace) -> int:
    image = pe_mod.PE(args.dll)
    channel, hooks = resolve_mod.load_rules(args.rules)
    resolver = resolve_mod.Resolver(image, hooks)

    entries: list[tomlio.PatternEntry] = []
    failures: list[str] = []
    for name in resolver.order:
        try:
            entry = resolver.resolve_one(name)
        except resolve_mod.RuleError as exc:
            failures.append(f"{name}: {exc}")
            continue
        entries.append(entry)

    print(resolver.explain(entries))
    if failures:
        print("\nUNRESOLVED (nothing published for these):", file=sys.stderr)
        for line in failures:
            print("  -", line, file=sys.stderr)

    if args.out_dir:
        dll_sha = sha256_of(image.path)
        out = Path(args.out_dir) / channel / f"{dll_sha}.toml"
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(tomlio.dump_pattern(entries), encoding="utf-8")
        print(f"\nwrote {out} ({len(entries)} hooks, dll sha256 {dll_sha})")

    if args.require_all and failures:
        print(
            f"\nFAILED: {len(failures)} hook(s) have no rule; "
            "refusing to call this build covered",
            file=sys.stderr,
        )
        return 1
    return 0


def cmd_verify(args: argparse.Namespace) -> int:
    image = pe_mod.PE(args.dll)
    channel, hooks = resolve_mod.load_rules(args.rules)
    truth = tomlio.load_pattern(args.truth)
    resolver = resolve_mod.Resolver(image, hooks)
    space = sig_mod.scan_space(image)

    missing = [n for n in truth if n not in {h.name for h in hooks}]
    print(f"ground truth hooks: {len(truth)}   rules: {len(hooks)}")
    if missing:
        print("without a rule:", ", ".join(sorted(missing)))

    bad = 0
    for name in truth:
        if name not in {h.name for h in hooks}:
            continue
        try:
            entry = resolver.resolve_one(name)
        except resolve_mod.RuleError as exc:
            print(f"MISS {name}: {exc}")
            bad += 1
            continue
        expected = truth[name]
        try:
            got_sig = sig_mod.signature_at(image, entry.rva, space=space)
        except sig_mod.SigError as exc:
            print(f"MISS {name}: sig error {exc}")
            bad += 1
            continue
        if entry.rva != expected.rva:
            print(f"RVA! {name}: got 0x{entry.rva:X} want 0x{expected.rva:X}")
            bad += 1
        elif got_sig != expected.sig:
            print(f"SIG! {name}: signature differs")
            print(f"     got : {got_sig}")
            print(f"     want: {expected.sig}")
            bad += 1
        else:
            print(f"  ok {name:<32} 0x{entry.rva:<8X} {resolver.resolved[name].via}")

    covered = len(truth) - len(missing)
    print(f"\ncovered by rules: {covered}/{len(truth)}   mismatches: {bad}")
    return 0 if bad == 0 else 1


def cmd_ipc(args: argparse.Namespace) -> int:
    """Generate the ipc-channel TOML (wire constants recovered from the DLL)."""
    image = pe_mod.PE(args.dll)
    idl_text = Path(args.idl).read_text(encoding="utf-8")
    interfaces, failures = ipc_mod.build(image, idl_text)
    total = sum(len(i.methods) for i in interfaces)
    print(f"resolved {total} methods across {len(interfaces)} interfaces")
    if failures:
        print(f"\nUNRESOLVED ({len(failures)}):", file=sys.stderr)
        for line in failures:
            print("  -", line, file=sys.stderr)

    if args.out_dir:
        dll_sha = sha256_of(image.path)
        out = Path(args.out_dir) / "steamclient" / f"{dll_sha}.toml"
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(ipc_mod.dump(interfaces), encoding="utf-8")
        print(f"\nwrote {out} (dll sha256 {dll_sha})")

    if args.verify:
        import tomllib

        raw = tomllib.loads(Path(args.verify).read_text(encoding="utf-8"))
        truth: dict[tuple[str, str], tuple[int, int, int]] = {}
        for iface_name, table in raw.items():
            if not isinstance(table, dict):
                continue
            for method_name, mtable in table.items():
                if not isinstance(mtable, dict):
                    continue
                truth[(iface_name, method_name)] = (
                    int(mtable.get("funcHash", "0x0"), 16),
                    int(mtable.get("fencepost", "0x0"), 16),
                    int(mtable.get("argc", 0)),
                )
        mine = {
            (i.name, m.name): (m.func_hash, m.fencepost, m.argc)
            for i in interfaces
            for m in i.methods
        }
        bad = 0
        for key, want in sorted(truth.items()):
            got = mine.get(key)
            if got is None:
                print(f"  MISS {key[0]}::{key[1]} (not resolved)")
                bad += 1
            elif got[0] != want[0] or got[1] != want[1]:
                print(
                    f"  DIFF {key[0]}::{key[1]} "
                    f"funcHash 0x{got[0]:08X}/0x{want[0]:08X} "
                    f"fencepost 0x{got[1]:08X}/0x{want[1]:08X}"
                )
                bad += 1
            else:
                print(f"  ok   {key[0]}::{key[1]}  hash=0x{got[0]:08X} argc={got[2]}")
        print(f"\nregression: {len(truth) - bad}/{len(truth)} exact")
        if bad:
            return 1
    return 0


def cmd_layout(args: argparse.Namespace) -> int:
    """Emit (or verify) SteamLayouts.gen.h from observed struct offsets."""
    values = layout_mod.NetPacketLayout(
        pub_data=args.pubdata,
        cub_data=args.cubdata,
        msghdr_emsg=args.msghdr_emsg,
        msghdr_hdrlen=args.msghdr_hdrlen,
    )
    if args.verify:
        problems = layout_mod.verify_header(args.verify, values)
        if problems:
            for line in problems:
                print("  DIFF", line, file=sys.stderr)
            return 1
        print(f"  ok   {args.verify} matches the given offsets")
        return 0
    if not args.out:
        print("nothing to do: pass --out <header> or --verify <header>", file=sys.stderr)
        return 2
    dll_sha = sha256_of(Path(args.dll)) if args.dll else "(unknown)"
    text = layout_mod.emit_header(
        values,
        dll_sha256=dll_sha,
        client_version=args.client_version or "",
        observed_at=args.observed_at or "",
    )
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text, encoding="utf-8")
    print(f"wrote {out} (pubData=0x{values.pub_data:X} cubData=0x{values.cub_data:X})")
    return 0


def cmd_survey(args: argparse.Namespace) -> int:
    image = pe_mod.PE(args.dll)
    truth = tomlio.load_pattern(args.truth)
    space = sig_mod.scan_space(image)
    print(f"# {image.path.name}: {len(truth)} hooks")
    for name, entry in sorted(truth.items()):
        fn = image.func_at(entry.rva) or image.func_of(entry.rva)
        span = f"0x{fn[0]:X}+{fn[1] - fn[0]}" if fn else "no .pdata entry"
        try:
            got = sig_mod.signature_at(image, entry.rva, space=space)
        except sig_mod.SigError as exc:
            got = f"<{exc}>"
        print(f"\n## {name} @0x{entry.rva:X} func={span} sig_ok={got == entry.sig}")
        if got != entry.sig:
            print(f"   want {entry.sig}\n   got  {got}")
        if fn:
            for target, text in list(image.strings_in_func(fn[0], fn[1], min_len=8).items())[:10]:
                refs = image.funcs_referencing(target)
                tag = "UNIQUE" if len(refs) == 1 else f"x{len(refs)}"
                print(f"   - {tag:<6} {text[:80]!r}")
        callers = image.callers_of(entry.rva)
        print(f"   callers: {len(callers)}")
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="ost-patterns", description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("resolve", help="resolve hooks and emit TOMLs")
    p.add_argument("dll")
    p.add_argument("--rules", required=True)
    p.add_argument("--out-dir")
    p.add_argument("--require-all", action="store_true")
    p.set_defaults(func=cmd_resolve)

    p = sub.add_parser("verify", help="diff resolver against ground truth")
    p.add_argument("dll")
    p.add_argument("truth")
    p.add_argument("--rules", required=True)
    p.set_defaults(func=cmd_verify)

    p = sub.add_parser("survey", help="inspect anchors around known hooks")
    p.add_argument("dll")
    p.add_argument("truth")
    p.set_defaults(func=cmd_survey)

    p = sub.add_parser("ipc", help="generate the ipc channel from a DLL + IDL")
    p.add_argument("dll")
    p.add_argument("--idl", required=True)
    p.add_argument("--out-dir")
    p.add_argument("--verify", help="third-party TOML used only as a regression oracle")
    p.set_defaults(func=cmd_ipc)

    p = sub.add_parser("layout", help="emit/verify SteamLayouts.gen.h")
    p.add_argument("--dll", help="steamclient64.dll the offsets were observed on")
    p.add_argument("--pubdata", type=lambda s: int(s, 0), default=0x10)
    p.add_argument("--cubdata", type=lambda s: int(s, 0), default=0x18)
    p.add_argument("--msghdr-emsg", type=lambda s: int(s, 0), default=0x00)
    p.add_argument("--msghdr-hdrlen", type=lambda s: int(s, 0), default=0x04)
    p.add_argument("--client-version", default="")
    p.add_argument("--observed-at", default="")
    p.add_argument("--out", help="header path to write")
    p.add_argument("--verify", help="header path to check against the given offsets")
    p.set_defaults(func=cmd_layout)

    args = ap.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
