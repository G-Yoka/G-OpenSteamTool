# OpenSteamTool Manager

OpenSteamTool Manager 鏄竴涓熀浜?`.NET 8 WPF` 鐨?Windows 妗岄潰绠＄悊鍣紝鐢ㄤ簬绠＄悊 Steam 鏍圭洰褰曚腑鐨?OpenSteamTool 鐩稿叧鏂囦欢銆丩ua 閰嶇疆銆佹父鎴忚祫婧愬拰鏇存柊娴佺▼銆?
## 鍔熻兘

- 閫夋嫨骞舵牎楠?Steam 鏍圭洰褰?- 瀹夎銆佺Щ闄ゅ苟鏍￠獙 `OpenSteamTool.dll`銆乣dwmapi.dll`銆乣xinput1_4.dll`
- 浣跨敤 SHA-256 瀵规瘮 Payload DLL 涓?Steam 鐩綍涓殑 DLL
- 妫€鏌?`OpenSteamTool.dll` 鏄惁宸插姞杞藉埌 `steam.exe`
- 鏄剧ず Steam 杩愯鐘舵€併€佺増鏈€丏LL 鐘舵€併€乀OML 鐘舵€併€丩ua 鐘舵€併€佹棩蹇楃姸鎬?- 缂栬緫 `opensteamtool.toml`
- 鎸夋瘡娓告垙涓€涓?Lua 鏂囦欢绠＄悊 `ost_<appid>.lua`
- 瀵煎叆鐜版湁 Lua 鏂囦欢骞舵帴绠′负绠＄悊鍣ㄩ厤缃?- 鏌ョ湅 `Steam\opensteamtool\*.log`
- 蹇€熼噸鍚?Steam
- 妫€鏌?GitHub Releases 鏇存柊
- 涓嬭浇骞跺簲鐢ㄧ鐞嗗櫒鏇存柊
- 鎸?Steam AppId 浠?GitHub 璧勬簮浠撳簱涓嬭浇瀹夎娓告垙璧勬簮

## 椤圭洰缁撴瀯

```text
OpenSteamTool.Manager/
  Views/
  ViewModels/
  Models/
  Services/
  Helpers/
  Converters/
  Payload/
```

褰撳墠淇濇寔鍗曢」鐩粨鏋勶紝涓嶆媶鍒嗙被搴撱€俉PF 鐣岄潰浣跨敤 MVVM 缁勭粐锛屾牳蹇冪涓夋柟渚濊禆浠呬负 `CommunityToolkit.Mvvm`銆?
## 鏋勫缓

```powershell
dotnet restore OpenSteamTool.Manager\OpenSteamTool.Manager.csproj
dotnet build OpenSteamTool.Manager\OpenSteamTool.Manager.csproj -c Release
```

鏋勫缓浜х墿浣嶄簬锛?
```text
OpenSteamTool.Manager\bin\Release\net8.0-windows\
```

## Payload DLL

搴旂敤瀹夎 DLL 鏃朵細璇诲彇锛?
```text
OpenSteamTool.Manager\Payload\
```

闇€瑕佸寘鍚細

- `OpenSteamTool.dll`
- `dwmapi.dll`
- `xinput1_4.dll`

## 绠＄悊鐨?Steam 鏂囦欢

- `Steam\OpenSteamTool.dll`
- `Steam\dwmapi.dll`
- `Steam\xinput1_4.dll`
- `Steam\opensteamtool.toml`
- `Steam\config\lua\ost_<appid>.lua`
- `Steam\opensteamtool\*.log`

## 璁稿彲璇?
鏈粨搴撻噰鐢?GNU General Public License v3.0锛岃瑙?[LICENSE](LICENSE)銆?
鏈」鐩寘鍚苟鍒嗗彂涓?OpenSteamTool 鐩稿叧鐨?DLL 璧勬簮锛孫penSteamTool 涓婃父椤圭洰涓?[OpenSteam001/OpenSteamTool](https://github.com/OpenSteam001/OpenSteamTool)锛屽叾璁稿彲璇佷负 GPL-3.0銆傜涓夋柟鏉ユ簮銆佷簩杩涘埗 Payload 鍜屽搴旇鏄庤 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)銆?
## 璇存槑

- 瀹夎鎴栫Щ闄?DLL 鍓嶉渶瑕佸叧闂?Steam銆?- Lua 鍜?TOML 鍙互鍦?Steam 杩愯鏃剁紪杈戯紝浣嗛儴鍒嗛厤缃渶瑕侀噸鍚?Steam 鍚庢墠浼氶噸鏂板姞杞姐€?- 鈥淒LL 宸插姞杞解€濈姸鎬佹潵鑷?`steam.exe` 褰撳墠妯″潡鍒楄〃锛屼笉绛夊悓浜庢枃浠舵槸鍚﹀瓨鍦ㄣ€?- 鈥淪team 鐗堟湰鈥濅紭鍏堣鍙?`Steam\logs\connection_log.txt` 涓渶鏂扮殑 `Client version`銆?