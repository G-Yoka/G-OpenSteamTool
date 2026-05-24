using System.IO;

namespace OpenSteamTool.Manager.Services;

public sealed class SteamLocatorService
{
    private readonly string _settingsDir = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "OpenSteamTool.Manager");

    private string SettingsFile => Path.Combine(_settingsDir, "settings.txt");

    public string LoadLastPath()
    {
        try
        {
            return File.Exists(SettingsFile) ? File.ReadAllText(SettingsFile).Trim() : string.Empty;
        }
        catch
        {
            return string.Empty;
        }
    }

    public void SaveLastPath(string steamPath)
    {
        Directory.CreateDirectory(_settingsDir);
        File.WriteAllText(SettingsFile, steamPath.Trim());
    }

    public bool IsValidSteamPath(string steamPath)
        => !string.IsNullOrWhiteSpace(steamPath)
           && Directory.Exists(steamPath)
           && File.Exists(Path.Combine(steamPath, "steam.exe"));
}
