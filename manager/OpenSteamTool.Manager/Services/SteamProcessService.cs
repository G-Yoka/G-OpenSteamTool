using System.Diagnostics;
using System.ComponentModel;
using System.IO;

namespace OpenSteamTool.Manager.Services;

public sealed class SteamProcessService
{
    public bool IsSteamRunning()
        => Process.GetProcessesByName("steam").Length > 0;

    public bool? IsOpenSteamToolLoaded()
        => IsModuleLoaded("steam", "OpenSteamTool.dll");

    public void RestartSteam(string steamPath)
    {
        var steamExe = Path.Combine(steamPath, "steam.exe");
        if (!File.Exists(steamExe))
        {
            throw new FileNotFoundException("未找到 steam.exe。", steamExe);
        }

        if (IsSteamRunning())
        {
            Process.Start(new ProcessStartInfo
            {
                FileName = steamExe,
                Arguments = "-shutdown",
                UseShellExecute = true,
                CreateNoWindow = true
            });

            var deadline = DateTime.UtcNow.AddSeconds(20);
            while (IsSteamRunning() && DateTime.UtcNow < deadline)
            {
                Thread.Sleep(250);
            }
        }

        Process.Start(new ProcessStartInfo
        {
            FileName = steamExe,
            UseShellExecute = true,
            CreateNoWindow = true
        });
    }

    private static bool? IsModuleLoaded(string processName, string moduleFileName)
    {
        var processes = Process.GetProcessesByName(processName);
        if (processes.Length == 0)
        {
            return null;
        }

        var inspected = false;

        foreach (var process in processes)
        {
            using (process)
            {
                try
                {
                    if (process.HasExited)
                    {
                        continue;
                    }

                    inspected = true;
                    foreach (ProcessModule module in process.Modules)
                    {
                        if (string.Equals(Path.GetFileName(module.FileName), moduleFileName, StringComparison.OrdinalIgnoreCase))
                        {
                            return true;
                        }
                    }
                }
                catch (Win32Exception)
                {
                    continue;
                }
                catch (InvalidOperationException)
                {
                    continue;
                }
                catch (NotSupportedException)
                {
                    continue;
                }
            }
        }

        return inspected ? false : null;
    }
}
