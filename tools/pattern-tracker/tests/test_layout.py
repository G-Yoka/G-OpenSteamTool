"""SteamLayouts.gen.h generator: offsets are data, so they can be re-derived."""

import pytest

from ost_patterns import layout as layout_mod


def test_emit_and_parse_roundtrip():
    values = layout_mod.NetPacketLayout(pub_data=0x10, cub_data=0x18)
    text = layout_mod.emit_header(values, dll_sha256="deadbeef", client_version="1792618285")
    assert "MACHINE-GENERATED" in text
    assert "deadbeef" in text
    assert "OST_LAYOUT_NETPACKET_PUBDATA 0x10u" in text
    assert "OST_LAYOUT_NETPACKET_CUBDATA 0x18u" in text
    assert layout_mod.parse_header(text) == values


def test_verify_header_detects_drift(tmp_path):
    path = tmp_path / "SteamLayouts.gen.h"
    path.write_text(
        layout_mod.emit_header(
            layout_mod.NetPacketLayout(pub_data=0x08, cub_data=0x10), dll_sha256="x"
        ),
        encoding="utf-8",
    )
    problems = layout_mod.verify_header(
        path, layout_mod.NetPacketLayout(pub_data=0x10, cub_data=0x18)
    )
    assert len(problems) == 2
    assert all("expected" in p for p in problems)


def test_implausible_offsets_rejected():
    with pytest.raises(layout_mod.LayoutError):
        layout_mod.NetPacketLayout(pub_data=0x1000, cub_data=0x18).validate()


def test_misaligned_pointer_offset_rejected():
    with pytest.raises(layout_mod.LayoutError):
        layout_mod.NetPacketLayout(pub_data=0x12, cub_data=0x1C).validate()
