using System.Diagnostics;
using System.IO;
using System.IO.Compression;
using System.Net.Http;
using System.Reflection;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using OpenSteamTool.Manager.Models;

namespace OpenSteamTool.Manager.Services;

public sealed class UpdateService
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNameCaseInsensitive = true
    };

    private readonly GitHubHttpService github;
    private readonly Version currentVersion;

    public UpdateService(GitHubHttpService github)
    {
        this.github = github;
        currentVersion = ReadCurrentVersion();
    }

    public string CurrentVersionText => currentVersion.ToString(3);

    public string RepositoryUrl => "https://github.com/G-Yoka/OpenSteamTool-Manager";

    public string ReleasesUrl => $"{RepositoryUrl}/releases";

    public async Task<UpdateCheckResult> CheckLatestAsync(CancellationToken cancellationToken = default)
    {
        var apiUrl = "https://api.github.com/repos/G-Yoka/OpenSteamTool-Manager/releases?per_page=1";

        try
        {
            using var request = github.CreateRequest(HttpMethod.Get, apiUrl);
            using var response = await github.SendAsync(request, cancellationToken);

            if (response.StatusCode == System.Net.HttpStatusCode.NotFound)
            {
                return CreateNoReleaseResult();
            }

            if (!response.IsSuccessStatusCode)
            {
                return CreateFailureResult(await github.BuildFailureMessageAsync(response, "GitHub Releases API request failed", cancellationToken));
            }

            await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
            var releases = await JsonSerializer.DeserializeAsync<GitHubReleaseDto[]>(stream, JsonOptions, cancellationToken)
                ?? Array.Empty<GitHubReleaseDto>();

            var release = releases.FirstOrDefault(x => !x.Draft && !x.Prerelease);
            if (release is null)
            {
                return CreateNoReleaseResult();
            }

            var latestVersion = ParseVersion(release.TagName);
            if (latestVersion is null)
            {
                return new UpdateCheckResult
                {
                    CurrentVersion = currentVersion,
                    HasRelease = true,
                    ReleaseName = release.Name ?? release.TagName ?? string.Empty,
                    ReleaseTag = release.TagName ?? string.Empty,
                    PublishedAt = release.PublishedAt,
                    ReleasePageUrl = release.HtmlUrl ?? ReleasesUrl,
                    ErrorMessage = $"Release tag {release.TagName} cannot be parsed as a SemVer version."
                };
            }

            var asset = FindUpdateAsset(release.Assets);

            return new UpdateCheckResult
            {
                CurrentVersion = currentVersion,
                LatestVersion = latestVersion,
                ReleaseName = release.Name ?? release.TagName ?? string.Empty,
                ReleaseTag = release.TagName ?? string.Empty,
                PublishedAt = release.PublishedAt,
                ReleasePageUrl = release.HtmlUrl ?? ReleasesUrl,
                AssetName = asset?.Name ?? string.Empty,
                AssetDownloadUrl = asset?.BrowserDownloadUrl ?? string.Empty,
                HasRelease = true,
                IsUpdateAvailable = latestVersion > currentVersion
            };
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (Exception ex)
        {
            return CreateFailureResult($"GitHub Releases API 闂備浇宕垫慨鏉懨洪銏犵哗闂侇剙绉甸崕鎴︽煟濡も偓閻楀棛娆㈤悙鍝勭骇闁割偒鍋勬禍顖滄偖? {ex.Message}");
        }
    }

    public async Task StartSelfUpdateAsync(
        UpdateCheckResult release,
        string installDirectory,
        int processId,
        IProgress<UpdateProgress>? progress = null,
        CancellationToken cancellationToken = default)
    {
        if (!release.HasRelease)
        {
            throw new InvalidOperationException("No public Release is available for auto update.");
        }

        if (string.IsNullOrWhiteSpace(release.AssetDownloadUrl))
        {
            throw new InvalidOperationException("This Release does not provide a ZIP asset for auto update.");
        }

        var workDir = Path.Combine(Path.GetTempPath(), "OpenSteamTool.Manager", "update", DateTime.UtcNow.ToString("yyyyMMddHHmmss"));
        var downloadDir = Path.Combine(workDir, "download");
        var extractDir = Path.Combine(workDir, "extract");
        Directory.CreateDirectory(downloadDir);
        Directory.CreateDirectory(extractDir);

        var zipPath = Path.Combine(downloadDir, release.AssetName);
        var sourceRoot = await DownloadAssetAsync(release.AssetDownloadUrl, zipPath, progress, cancellationToken);

        progress?.Report(new UpdateProgress { Stage = "Extracting", Percent = 0, Message = "Preparing update files..." });
        ZipFile.ExtractToDirectory(sourceRoot, extractDir, overwriteFiles: true);

        var payloadRoot = ResolvePayloadRoot(extractDir);
        var exeName = Path.GetFileName(Environment.ProcessPath ?? "OpenSteamTool.Manager.exe");
        var updateScript = Path.Combine(workDir, "apply-update.ps1");

        await File.WriteAllTextAsync(updateScript, BuildScript(), new UTF8Encoding(false), cancellationToken);
        StartHelperProcess(updateScript, payloadRoot, installDirectory, exeName, processId);
        progress?.Report(new UpdateProgress { Stage = "Replacing", Percent = 100, Message = "Update helper started; the app will replace files and restart after exit." });
    }

    private UpdateCheckResult CreateNoReleaseResult()
        => new()
        {
            CurrentVersion = currentVersion,
            HasRelease = false
        };

    private UpdateCheckResult CreateFailureResult(string message)
        => new()
        {
            CurrentVersion = currentVersion,
            HasRelease = false,
            ErrorMessage = message
        };

    private static Version? ParseVersion(string? tagName)
    {
        if (string.IsNullOrWhiteSpace(tagName))
        {
            return null;
        }

        var trimmed = tagName.Trim().TrimStart('v', 'V');
        if (Version.TryParse(trimmed, out var version))
        {
            return Normalize(version);
        }

        return null;
    }

    private static Version Normalize(Version version)
        => new(version.Major, version.Minor, Math.Max(version.Build, 0), Math.Max(version.Revision, 0));

    private static GitHubReleaseAssetDto? FindUpdateAsset(IReadOnlyList<GitHubReleaseAssetDto> assets)
    {
        if (assets.Count == 0)
        {
            return null;
        }

        return assets.FirstOrDefault(x =>
                   x.Name.EndsWith(".zip", StringComparison.OrdinalIgnoreCase)
                   && x.Name.Contains("OpenSteamTool.Manager", StringComparison.OrdinalIgnoreCase))
               ?? assets.FirstOrDefault(x => x.Name.EndsWith(".zip", StringComparison.OrdinalIgnoreCase));
    }

    private async Task<string> DownloadAssetAsync(
        string url,
        string zipPath,
        IProgress<UpdateProgress>? progress,
        CancellationToken cancellationToken)
    {
        using var request = github.CreateRequest(HttpMethod.Get, url, "application/octet-stream");
        using var response = await github.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken);
        if (!response.IsSuccessStatusCode)
        {
            throw new InvalidOperationException(await github.BuildFailureMessageAsync(response, "GitHub release asset download failed", cancellationToken));
        }

        await using var input = await response.Content.ReadAsStreamAsync(cancellationToken);
        await using var output = File.Create(zipPath);

        var totalLength = response.Content.Headers.ContentLength;
        var buffer = new byte[81920];
        long readTotal = 0;
        int read;
        while ((read = await input.ReadAsync(buffer.AsMemory(0, buffer.Length), cancellationToken)) > 0)
        {
            await output.WriteAsync(buffer.AsMemory(0, read), cancellationToken);
            readTotal += read;

            if (totalLength.HasValue && totalLength.Value > 0)
            {
                var percent = (int)Math.Clamp(readTotal * 100L / totalLength.Value, 0, 100);
                progress?.Report(new UpdateProgress
                {
                    Stage = "Downloading",
                    Percent = percent,
                    Message = $"濠电姵顔栭崰妤冩崲閹邦喖绶ら柦妯侯檧閼版寧銇勮箛鎾村櫧闁崇粯妫冮幃妤呮晲鎼粹€崇缂備椒妞掗崡鎶藉蓟閿熺姴鐐婄憸蹇涘箺閻樼粯鐓涢悘鐐存灮闊剛鈧?.. {percent}%"
                });
            }
        }

        progress?.Report(new UpdateProgress
        {
            Stage = "Downloading",
            Percent = 100,
            Message = "Update package download completed."
        });

        return zipPath;
    }

    private static string ResolvePayloadRoot(string extractDir)
    {
        if (File.Exists(Path.Combine(extractDir, "OpenSteamTool.Manager.exe")))
        {
            return extractDir;
        }

        var directories = Directory.EnumerateDirectories(extractDir).ToList();
        if (directories.Count == 1)
        {
            var nested = directories[0];
            if (File.Exists(Path.Combine(nested, "OpenSteamTool.Manager.exe")))
            {
                return nested;
            }
        }

        return extractDir;
    }

    private static void StartHelperProcess(string scriptPath, string sourceDir, string targetDir, string exeName, int processId)
    {
        var powershell = Path.Combine(Environment.SystemDirectory, "WindowsPowerShell", "v1.0", "powershell.exe");
        if (!File.Exists(powershell))
        {
            powershell = "powershell.exe";
        }

        var startInfo = new ProcessStartInfo
        {
            FileName = powershell,
            UseShellExecute = false,
            CreateNoWindow = true,
            WindowStyle = ProcessWindowStyle.Hidden
        };

        startInfo.ArgumentList.Add("-NoProfile");
        startInfo.ArgumentList.Add("-ExecutionPolicy");
        startInfo.ArgumentList.Add("Bypass");
        startInfo.ArgumentList.Add("-File");
        startInfo.ArgumentList.Add(scriptPath);
        startInfo.ArgumentList.Add("-Source");
        startInfo.ArgumentList.Add(sourceDir);
        startInfo.ArgumentList.Add("-Target");
        startInfo.ArgumentList.Add(targetDir);
        startInfo.ArgumentList.Add("-ExeName");
        startInfo.ArgumentList.Add(exeName);
        startInfo.ArgumentList.Add("-ProcessId");
        startInfo.ArgumentList.Add(processId.ToString());

        using var helper = Process.Start(startInfo) ?? throw new InvalidOperationException("Failed to start update helper process.");
    }

    private static Version ReadCurrentVersion()
    {
        var entry = Assembly.GetEntryAssembly() ?? Assembly.GetExecutingAssembly();
        var informational = entry.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion;
        if (!string.IsNullOrWhiteSpace(informational))
        {
            var trimmed = informational.Trim().TrimStart('v', 'V');
            if (Version.TryParse(trimmed, out var version))
            {
                return Normalize(version);
            }
        }

        var versionInfo = entry.GetName().Version ?? new Version(0, 0, 0, 0);
        return Normalize(versionInfo);
    }

    private static string BuildScript()
        => """
param(
    [Parameter(Mandatory = $true)][string]$Source,
    [Parameter(Mandatory = $true)][string]$Target,
    [Parameter(Mandatory = $true)][string]$ExeName,
    [Parameter(Mandatory = $true)][int]$ProcessId
)

$ErrorActionPreference = 'Stop'

while (Get-Process -Id $ProcessId -ErrorAction SilentlyContinue) {
    Start-Sleep -Milliseconds 500
}

if (-not (Test-Path -LiteralPath $Source)) {
    throw "闂傚倷绀侀幖顐⒚洪妶澶嬪仱闁靛ň鏅涢拑鐔封攽閻樻彃鈧敻寮ㄩ敃鍌涚厵闂侇叏绠戦弸鐔虹磼閹邦収娈橀柟鍙夋倐閹囧醇閻旂尨绱辩紓鍌欑劍椤ㄥ懘藝闁秴鐒垫い鎺嶈兌閳洟鏌ㄥ顓犵瘈闁? $Source"
}

New-Item -ItemType Directory -Path $Target -Force | Out-Null

& "$env:SystemRoot\System32\robocopy.exe" $Source $Target /MIR /R:2 /W:1 /NFL /NDL /NJH /NJS /NP | Out-Null
if ($LASTEXITCODE -ge 8) {
    throw "Robocopy 婵犵數濮伴崹娲磿閼测晛鍨濋柛鎾楀嫬鏋傞梺鎸庢礀閸婂綊寮查鍌楀亾閸忓浜鹃梺鍛婃磸閸斿本绂嶆ィ鍐╃厓鐟滄粓宕滈悢椋庢殾婵炲棙鎸婚幆鐐烘煕閿旇骞栭柣? $LASTEXITCODE"
}

$exePath = Join-Path $Target $ExeName
if (-not (Test-Path -LiteralPath $exePath)) {
    throw "闂傚倷绀侀幖顐⒚洪妶澶嬪仱闁靛ň鏅涢拑鐔封攽閻樺弶鎼愰悷娆欑畵楠炴牗娼忛崜褏蓱闂佷紮缍€娴滎剛妲愰幒鏂哄亾閿濆簼鎲鹃柛搴＄箳缁辨帡鈥﹂幋婵嗙睄閻庢鍠氶弫濠氥€侀弮鍫濆窛妞ゆ挆鍕垫（闂? $exePath"
}

Start-Process -FilePath $exePath -WorkingDirectory $Target | Out-Null
""";

    private sealed class GitHubReleaseDto
    {
        [JsonPropertyName("name")]
        public string? Name { get; set; }

        [JsonPropertyName("tag_name")]
        public string? TagName { get; set; }

        [JsonPropertyName("html_url")]
        public string? HtmlUrl { get; set; }

        [JsonPropertyName("published_at")]
        public DateTimeOffset? PublishedAt { get; set; }

        [JsonPropertyName("draft")]
        public bool Draft { get; set; }

        [JsonPropertyName("prerelease")]
        public bool Prerelease { get; set; }

        [JsonPropertyName("assets")]
        public List<GitHubReleaseAssetDto> Assets { get; set; } = new();
    }

    private sealed class GitHubReleaseAssetDto
    {
        [JsonPropertyName("name")]
        public string Name { get; set; } = string.Empty;

        [JsonPropertyName("browser_download_url")]
        public string BrowserDownloadUrl { get; set; } = string.Empty;
    }
}
