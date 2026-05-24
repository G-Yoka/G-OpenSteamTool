using System.IO;
using System.Security.Cryptography;
using OpenSteamTool.Manager.Models;

namespace OpenSteamTool.Manager.Services;

public sealed class PayloadService
{
    public static readonly string[] ManagedDlls =
    [
        "OpenSteamTool.dll",
        "dwmapi.dll",
        "xinput1_4.dll"
    ];

    private readonly string _payloadDir = Path.Combine(AppContext.BaseDirectory, "Payload");

    public IReadOnlyList<DllInstallStatus> GetDllStatuses(string steamPath)
    {
        return ManagedDlls.Select(name =>
        {
            var target = Path.Combine(steamPath, name);
            var payload = Path.Combine(_payloadDir, name);
            var backup = GetBackupPath(steamPath, name);
            var payloadAvailable = File.Exists(payload);
            var installed = File.Exists(target);
            var payloadHash = payloadAvailable ? ComputeSha256(payload) : null;
            var targetHash = installed ? ComputeSha256(target) : null;

            return new DllInstallStatus
            {
                Name = name,
                TargetPath = target,
                PayloadAvailable = payloadAvailable,
                Installed = installed,
                MatchesPayload = payloadAvailable && installed && payloadHash == targetHash,
                HasBackup = File.Exists(backup),
                PayloadHash = payloadHash,
                TargetHash = targetHash
            };
        }).ToList();
    }

    public void Install(string steamPath, bool steamRunning)
    {
        if (steamRunning)
        {
            throw new InvalidOperationException("Steam 正在运行，请先关闭 Steam 再安装 DLL。");
        }

        Directory.CreateDirectory(GetBackupDir(steamPath));

        foreach (var name in ManagedDlls)
        {
            var payload = Path.Combine(_payloadDir, name);
            if (!File.Exists(payload))
            {
                throw new FileNotFoundException($"缺少内置 DLL：{payload}", payload);
            }

            var target = Path.Combine(steamPath, name);
            var backup = GetBackupPath(steamPath, name);
            if (File.Exists(target) && !HashesMatch(target, payload) && !File.Exists(backup))
            {
                File.Copy(target, backup, overwrite: false);
            }

            File.Copy(payload, target, overwrite: true);
        }
    }

    public void Remove(string steamPath, bool steamRunning)
    {
        if (steamRunning)
        {
            throw new InvalidOperationException("Steam 正在运行，请先关闭 Steam 再移除 DLL。");
        }

        foreach (var name in ManagedDlls)
        {
            var payload = Path.Combine(_payloadDir, name);
            var target = Path.Combine(steamPath, name);
            var backup = GetBackupPath(steamPath, name);

            if (File.Exists(backup))
            {
                File.Copy(backup, target, overwrite: true);
                File.Delete(backup);
                continue;
            }

            if (File.Exists(target) && File.Exists(payload) && HashesMatch(target, payload))
            {
                File.Delete(target);
            }
        }
    }

    private static string GetBackupDir(string steamPath)
        => Path.Combine(steamPath, "opensteamtool-manager", "backup");

    private static string GetBackupPath(string steamPath, string fileName)
        => Path.Combine(GetBackupDir(steamPath), fileName);

    private static bool HashesMatch(string left, string right)
        => ComputeSha256(left) == ComputeSha256(right);

    private static string ComputeSha256(string path)
    {
        using var stream = File.OpenRead(path);
        var hash = SHA256.HashData(stream);
        return Convert.ToHexString(hash);
    }
}
