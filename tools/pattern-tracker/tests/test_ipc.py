"""ipc channel generator: IDL parsing and wire-constant extraction."""

import struct

from ost_patterns import ipc as ipc_mod


def test_parse_idl_reads_interface_ids_and_arity():
    text = """
enum EIPCInterface : uint8 {
    IClientUser  = 1;
    IClientUtils = 4;
};

interface IClientUser {
    CSteamID GetSteamID();

    uint32 GetAppOwnershipTicketExtendedData(
        in  uint32 unAppID,
        in  int32  cbMaxTicket,
        out bytes  pTicket[cbMaxTicket],
        out uint32 piAppId
    );
};

interface IClientUtils {
    bool GetAPICallResult(
        in  uint64 hSteamAPICall,
        in  uint32 cubCallback,
        in  uint32 iCallbackExpected,
        out bytes  pCallback[cubCallback],
        out bool   pbFailed
    );
};
"""
    interfaces = {i.name: i for i in ipc_mod.parse_idl(text)}
    assert interfaces["IClientUser"].interface_id == 1
    assert interfaces["IClientUtils"].interface_id == 4
    user = {m.name: m for m in interfaces["IClientUser"].methods}
    assert user["GetSteamID"].argc == 0
    # argc counts incoming parameters only
    assert user["GetAppOwnershipTicketExtendedData"].argc == 2
    utils = {m.name: m for m in interfaces["IClientUtils"].methods}
    assert utils["GetAPICallResult"].argc == 3


def test_wire_constants_picks_up_stack_immediates():
    # mov dword ptr [rbp+40h], 0D6FC3200h   /   mov dword ptr [rbp+40h], 0D7058CA5h
    body = (
        b"\x90" * 8
        + b"\xc7\x45\x40" + struct.pack("<I", 0xD6FC3200)
        + b"\x90" * 8
        + b"\xc7\x45\x40" + struct.pack("<I", 0xD7058CA5)
        + b"\xc3"
    )
    found = ipc_mod.wire_constants(None, body)
    assert [v for _off, v in found] == [0xD6FC3200, 0xD7058CA5]


def test_wire_constants_handles_rsp_disp32_form():
    # mov dword ptr [rsp+80h], imm32 -- the wider encoding on larger frames
    body = (
        b"\x90" * 4
        + b"\xc7\x84\x24\x80\x00\x00\x00" + struct.pack("<I", 0x25D6BB1D)
        + b"\xc3"
    )
    found = ipc_mod.wire_constants(None, body)
    assert found and found[0][1] == 0x25D6BB1D


def test_dump_emits_loader_compatible_layout():
    iface = ipc_mod.IpcInterface(name="IClientUser", interface_id=1)
    method = ipc_mod.IpcMethod(
        name="GetSteamID", argc=0, func_hash=0xD6FC3200,
        fencepost=0xD7058CA5, wrapper_rva=0x7A23C0,
    )
    iface.methods.append(method)
    text = ipc_mod.dump([iface])
    assert "[IClientUser]" in text
    assert "interface_id = 1" in text
    assert "[IClientUser.GetSteamID]" in text
    assert 'funcHash = "0xD6FC3200"' in text
    assert 'fencepost = "0xD7058CA5"' in text
    assert 'wrapper_rva = "0x7A23C0"' in text
