# pattern-tracker

Zero-seed signature pipeline for OpenSteamTool pattern TOMLs: **DLL in, TOMLs out,
no prior pattern file needed**.

It exists because the canonical pattern feed
(`OpenSteam001/steam-monitor`, `pattern` branch) has not been updated since
2026-09-06, while Steam shipped new builds (e.g. `steamui.dll`
`1b945794…b1252`, `steamclient64.dll` `e9045710…eb4e`, both 2026-10-14) that have
no published TOMLs. `OpenSteamTool.dll` therefore disables the hooks it cannot
match and shows an "Unsupported Steam Version" popup per DLL. This tool derives
the missing patterns instead of copying somebody else's feed.

## What it actually does

For each hook name in `rules/<channel>.toml` it derives a function entry RVA,
then synthesises the published signature:

```
literal ("anchor" string in .rdata)
      └─ lea xref in .text  ──► containing .pdata function ──► entry RVA
                                    │
                                    └─ sig = bytes at entry (see below)
```

* **`.pdata` is the authority** for function boundaries. Leaf functions without
  unwind info (e.g. `GetTopManager`, `GetPipeClient`) fall back to a
  pessimistic backwards scan that only accepts a start preceded by a return or
  `int3` padding.
* **Signature rule** (reverse-verified against published feeds, byte for byte):
  shortest prefix that occurs exactly once in the scan space (`.text`), but
  **never shorter than 24 bytes** and capped at 80. If even 80 bytes are
  ambiguous the hook fails instead of publishing a pattern that could match the
  wrong function.
* **Nothing is guessed.** Every rule must yield exactly one candidate, or the
  hook is reported `UNRESOLVED` and no file is written for it.

### Rule kinds

| kind | predicate |
| --- | --- |
| `anchor` | the function references every listed literal (exact NUL-terminated ASCII) |
| `callers` | direct callers of another hook (`of`) or of a literal-anchored function (`of_anchor`); optional `strings` / `span` / `leaf_only` / `min_callers` / `depth` filters |
| `callee` | direct callees of the same bases; same filters |
| `vtable_slot` | `class_name` + `slot`: MSVC RTTI → CompleteObjectLocator → vtable → slot |
| `global_thunk` | 8-byte `mov rax,[rip+G]; ret` getter, where `G` is a global the anchored function also touches |
| `same_as` | alias of another hook (one function serving two roles) |

The filters are what made the anonymous helpers tractable, because such a hook
has no literal, no vtable slot and no meaningful call-graph neighbourhood of its
own:

* `span` — exact `.pdata` byte length. "The *36-byte* callee of
  `FillInAppOverview`" is deterministic and cheap to re-verify.
* `leaf_only` — the candidate must have no `.pdata` entry (tiny getters and
  thunks). Strict on purpose: ambiguous the moment a second leaf appears.
* `min_callers` — popularity floor, used only where the runner-up is an order of
  magnitude away (`GetAppIDForCurrentPipe`: 286 vs 15). It fails loudly both when
  the intended function drops below the floor and when a second candidate
  crosses it.

`vtable_slot` and `global_thunk` cover hooks that have no call edges at all:
`CSteamUIAppController::RunFrame`, `CCMInterface::RecvPkt` and
`CConfigStore`'s binary reader are only ever invoked through a vtable, and
`GetTopManager` is a lone 8-byte singleton getter.

## The two data channels

The DLL consumes two independent channels; both are now derivable from the
binaries alone.

### `pattern` — where a function lives (see above)

### `ipc` — which interface/method a wire call means

`pattern` alone leaves every IPC handler disabled:
`Hooks_IPC: 0 handlers registered` / `[Handler Disabled] no IPC spec for
IClientUser::GetSteamID`. The IPC layer matches calls coming over the wire by
`(interfaceID, funcHash)`, so it needs the **wire constants** per method.

Recovered without any prior data file, from `steamclient64.dll`:

```
method-name literal  ──►  the one function referencing it    (the wrapper)
interface-name literal ─►  also referenced there             (groups by interface)
wrapper body         ──►  c7 45 40 <imm32>   mov [rbp+40h], funcHash
                          c7 45 40 <imm32>   mov [rbp+40h], fencepost
```

`argc` comes from our own IDL (`IPCMessages.steamd`, number of `in` params) and
`interface_id` from its `EIPCInterface` enum.

```powershell
python -m ost_patterns.cli ipc D:\Steam\steamclient64.dll `
    --idl path\to\IPCMessages.steamd --out-dir out-ipc `
    --verify fixtures\local\ipc_steamclient_<sha256>.toml
```

`--verify` takes a third-party TOML and diffs every `funcHash` / `fencepost` /
`argc`. Current result on the live install: **6/6 exact**, and the loader then
reports `Hooks_IPC: 6 handlers registered` instead of `0`.

### `layout` — how a struct is laid out in the current client

The third piece of volatile knowledge. `CNetPacket`'s `m_pubData` / `m_cubData`
moved by 8 bytes in the 2026-10-22 build; reading them at the old offsets made
the `RecvPkt` hook feed a bogus pointer into the protobuf parser (`AV` reading
`0xFFFFFFFF`) — dozens of suppressed exceptions per session and, before the SEH
guard, a dead client.

The offsets now live in generated data, `src/Steam/SteamLayouts.gen.h`, and all
access goes through the `SteamLayouts::NetPacket*` / `MsgHdr*` helpers in
`Structs.h`. Update path:

```powershell
# 1. re-observe (Debug build with /DOST_LAYOUT_PROBE logs the packet object)
# 2. regenerate the header from the observed values
python -m ost_patterns.cli layout --dll D:\Steam\steamclient64.dll `
    --pubdata 0x10 --cubdata 0x18 --client-version <version> `
    --observed-at <date> --out path\to\src\Steam\SteamLayouts.gen.h

# 3. CI gate: fail if the checked-in header disagrees with the expected values
python -m ost_patterns.cli layout --pubdata 0x10 --cubdata 0x18 `
    --verify path\to\src\Steam\SteamLayouts.gen.h
```

Measured effect on the 2026-10-22 client (Debug build, 70 s session):

| | before | after |
| --- | --- | --- |
| `RecvPkt` exceptions (`0xC0000005`) | dozens | **0** |
| inbound messages parsed (`Recv eMsg …`) | 0 | **244** |

## How the loader consumes the file

Two things matter, and only one of them is obvious:

1. **The file name** must be `<sha256 of the DLL>.toml`.
2. **The table keys** must be `FNV-1a 32-bit` of the hook name, e.g.
   `GetAppByID` → `0x3FC68546`. `OpenSteamTool.dll` looks patterns up by that
   key, not by name.

Getting (2) wrong is silent and nasty: the file loads, the loader reports
`loaded 10 patterns for steamui`, and then every hook fails with
`PatternLoader: no entry for 'GetAppByID' (key=0x3FC68546)` — the popups stay
and nothing is patched. This pipeline originally invented a SHA-256 based key
and only the real loader caught it; `tests/test_tomlio.py::test_hook_key_matches_loader`
now pins twelve name→key pairs taken from the loader's own log lines.

## Status (measured, not aspirational)

Validated three ways on a real install:

| check | result |
| --- | --- |
| previous Steam build vs published feed | 10 + 25 hooks, identical names, RVAs and signatures |
| **next** Steam build (2026-10-22, no feed had it) | 35/35 hooks resolved with **zero rule changes** |
| loader on the live install | `loaded 10 patterns for steamui` / `loaded 25 patterns for steamclient`, **0** `no entry` warnings, **0** `no source available` |

The second row is the point of the whole exercise: when Steam updated, the same
rule set re-derived every hook against binaries nobody had rules for, and the
loader accepted all 35 patterns (no key misses, no signature misses).

Independent cross-check against a community feed that had already published the
new build: 10/10 `steamui` RVAs and 23/25 `steamclient` RVAs agree exactly. The
two disagreements (`BuildSpawnEnvBlock`, `SendCallbackToPipe`) are explained by
that feed targeting a **newer OpenSteamTool** version — it carries an extra hook
(`YldLoadDepotManifest`) and points those two names at different functions
(`SendCallbackToPipe` there has 0 callers and does not touch the pipe client).
For the installed 1.4.8 DLL our values are the ones that match, and they are the
ones that matched the published 1.4.8-era feed hook for hook.

## Usage

```powershell
# resolve hooks for a DLL and emit pattern/<channel>/<sha256>.toml
python -m ost_patterns.cli resolve D:\Steam\steamui.dll --rules rules\steamui.toml --out-dir out

# same, but exit non-zero when any hook is still unresolved (CI gate)
python -m ost_patterns.cli resolve D:\Steam\steamui.dll --rules rules\steamui.toml --out-dir out --require-all

# diff the resolver against a ground-truth TOML (RVA + signature)
python -m ost_patterns.cli verify D:\Steam\steamui.dll fixtures\local\<sha256>.toml --rules rules\steamui.toml

# inspect literals / callers / callees around known hooks while writing a rule
python -m ost_patterns.cli survey D:\Steam\steamui.dll fixtures\local\<sha256>.toml
```

Tests (the integration test is skipped unless the DLL and a ground-truth TOML
are supplied):

```powershell
python -m pytest tests -q
$env:OST_DLL="D:\Steam\steamui.dll"
$env:OST_TRUTH="fixtures\local\<sha256>.toml"
$env:OST_RULES="rules\steamui.toml"
python -m pytest tests\test_integration.py -q
```

## Not implemented yet

* **`fetch_client.py`** — downloading a *new* Steam build so CI can run without a
  workstation. Until it exists, resolution runs against a local Steam install
  (`<Steam>\steamui.dll`, `<Steam>\steamclient64.dll`) or manually supplied
  binaries, and `.github/workflows/pattern.yml` only runs the unit tests.
* The `ipc` channel. OpenSteamTool 1.4.8 fetches `pattern` metadata only; a
  future version that also asks for `ipc` will need the same treatment.
* Emitting `versions.json` / an aggregate index for a published mirror.

## Trust boundary and licensing

* This repository **ships no third-party pattern data**. `fixtures/local/` (the
  ground-truth TOMLs used by `verify`) is git-ignored on purpose: those files
  come from community feeds without a licence, and copying them into a
  published repo is a redistribution you may not be allowed to make.
* Generating your own data is the point: the output is derived from your own
  analysis of the DLL, so it is yours to publish.
* `OpenSteamTool.dll` itself is GPL-3.0. If a distribution of G-OpenSteamTool
  bundles that binary, the GPL obligations apply to that distribution; this tool
  only writes data files and does not modify or redistribute the DLL.
* Pattern TOMLs are security-relevant: they decide which RVAs get hooked. Never
  point `[remote] url_template` at a feed you do not trust, and never publish a
  rule whose predicate you have not verified against a real build.

## Repository layout

```
ost_patterns/
  pe.py        PE image: sections, strings, RIP-relative lea xrefs, .pdata
  sig.py       signature synthesis + unique-prefix trimming
  rtti.py      MSVC RTTI -> class vtable -> slot
  resolve.py   rule engine (strict: exactly one candidate)
  tomlio.py    pattern TOML / versions.json read + write
  cli.py       survey | resolve | verify
rules/         one TOML per channel: the accumulated, verified knowledge
tests/         unit tests + optional integration gate
```

This directory is self-contained and does not import anything from the Tauri app;
it can be moved to its own repository unchanged.
