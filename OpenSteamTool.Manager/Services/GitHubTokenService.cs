using System.IO;
using System.Security.Cryptography;
using System.Text;

namespace OpenSteamTool.Manager.Services;

public sealed class GitHubTokenService
{
    private readonly string tokenPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "OpenSteamTool.Manager",
        "github-token.dat");

    public bool HasToken => !string.IsNullOrWhiteSpace(GetToken());

    public void SaveToken(string token)
    {
        var trimmed = token.Trim();
        if (string.IsNullOrWhiteSpace(trimmed))
        {
            ClearToken();
            return;
        }

        var directory = Path.GetDirectoryName(tokenPath);
        if (!string.IsNullOrWhiteSpace(directory))
        {
            Directory.CreateDirectory(directory);
        }

        var plainBytes = Encoding.UTF8.GetBytes(trimmed);
        var protectedBytes = ProtectedData.Protect(plainBytes, optionalEntropy: null, DataProtectionScope.CurrentUser);
        File.WriteAllBytes(tokenPath, protectedBytes);
    }

    public void ClearToken()
    {
        if (File.Exists(tokenPath))
        {
            File.Delete(tokenPath);
        }
    }

    public string? GetToken()
    {
        try
        {
            if (!File.Exists(tokenPath))
            {
                return null;
            }

            var protectedBytes = File.ReadAllBytes(tokenPath);
            var plainBytes = ProtectedData.Unprotect(protectedBytes, optionalEntropy: null, DataProtectionScope.CurrentUser);
            return Encoding.UTF8.GetString(plainBytes).Trim();
        }
        catch
        {
            return null;
        }
    }
}
