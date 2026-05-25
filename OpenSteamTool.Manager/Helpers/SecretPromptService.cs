using System.Windows;
using OpenSteamTool.Manager.Views;

namespace OpenSteamTool.Manager.Helpers;

public sealed class SecretPromptService : ISecretPromptService
{
    public string? Show(string title, string message)
    {
        var window = new SecretPromptWindow(title, message)
        {
            Owner = System.Windows.Application.Current?.MainWindow
        };

        return window.ShowDialog() == true ? window.Value : null;
    }
}
