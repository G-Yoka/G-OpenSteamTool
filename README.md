# OpenSteamTool Manager

OpenSteamTool Manager 鏄竴涓熀浜?`.NET 8 WPF` 鐨?Windows 妗岄潰绠＄悊鍣紝鐢ㄦ潵绠＄悊 Steam 鏍圭洰褰曢噷鐨?OpenSteamTool 鐩稿叧鏂囦欢銆丩ua 閰嶇疆銆佹棩蹇椼€佹父鎴忚祫婧愬寘锛屼互鍙婂簲鐢ㄥ唴鏇存柊銆?
褰撳墠鐗堟湰锛歚v0.1.5`

## 涓昏鍔熻兘

- 閫夋嫨骞舵牎楠?Steam 鏍圭洰褰?- 瀹夎銆佺Щ闄ゅ苟鏍￠獙 `OpenSteamTool.dll`銆乣dwmapi.dll`銆乣xinput1_4.dll`
- 閫氳繃 SHA-256 鏍￠獙 Payload 涓?Steam 鐩綍涓殑 DLL 鏄惁涓€鑷?- 妫€鏌?`OpenSteamTool.dll` 鏄惁鐪熺殑宸插姞杞借繘 `steam.exe`
- 缂栬緫 `opensteamtool.toml`
- 鎸夋瘡涓父鎴忎竴涓?Lua 鏂囦欢绠＄悊 `ost_<appid>.lua`
- 瀵煎叆鐜版湁 Lua 鏂囦欢骞舵帴绠′负绠＄悊鍣ㄩ厤缃?- 鏌ョ湅 `Steam\\opensteamtool\\*.log`
- 蹇€熼噸鍚?Steam
- 鍦ㄥ簲鐢ㄥ唴妫€鏌ユ洿鏂般€佷笅杞藉苟鏇挎崲绋嬪簭
- 鎸?Steam AppId 浠?GitHub 娓告垙璧勬簮浠撳簱涓嬭浇骞跺畨瑁呰祫婧?- 鍒囨崲鏇存柊/璧勬簮鏉ユ簮浼樺厛绾э細`jsDelivr CDN` 鎴?`GitHub Releases`
- 娴嬭瘯 jsDelivr銆丟itHub API銆丟itHub Raw 鐨勮繛閫氭€?
## 鏇存柊鏂瑰紡

绠＄悊鍣ㄤ紭鍏堜娇鐢?`jsDelivr CDN` 鑾峰彇鏇存柊鍏冩暟鎹拰娓告垙璧勬簮娓呭崟锛孏itHub 浣滀负鍏滃簳鏉ユ簮銆?
- 鏇存柊鍏冩暟鎹細`cdn/update.json`
- 鏇存柊鍖咃細`releases/OpenSteamTool.Manager-vX.Y.Z.zip`
- 娓告垙璧勬簮娓呭崟锛歚GameResources/manifest.json`

濡傛灉浣犺鍙戝竷鏂扮増鏈紝璇峰悓姝ユ洿鏂帮細

- `OpenSteamTool.Manager/OpenSteamTool.Manager.csproj`
- `cdn/update.json`
- `releases/OpenSteamTool.Manager-vX.Y.Z.zip`

## 鏋勫缓

```powershell
dotnet restore OpenSteamTool.Manager\OpenSteamTool.Manager.csproj
dotnet build OpenSteamTool.Manager\OpenSteamTool.Manager.csproj -c Release
```

杈撳嚭鐩綍锛?
```text
OpenSteamTool.Manager\bin\Release\net8.0-windows\
```

## 鐩綍缁撴瀯

```text
OpenSteamTool.Manager/
  Views/
  ViewModels/
  Models/
  Services/
  Helpers/
  Converters/
  Payload/
  cdn/
  releases/
```

## 璁稿彲

鏈粨搴撻噰鐢?`GPL-3.0-only` 璁稿彲璇併€侽penSteamTool 鐩稿叧涓婃父椤圭洰鍚屾牱閬靛惊 GPL-3.0锛岀涓夋柟璧勬簮涓庤鏄庤 `THIRD_PARTY_NOTICES.md`銆?