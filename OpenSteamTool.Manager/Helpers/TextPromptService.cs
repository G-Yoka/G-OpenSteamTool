using System.Windows;
using OpenSteamTool.Manager.ViewModels;
using OpenSteamTool.Manager.Views;

namespace OpenSteamTool.Manager.Helpers;

public sealed class TextPromptService : ITextPromptService
{
    public string? Show(string title, string message, string defaultValue)
    {
        var viewModel = new TextPromptViewModel(title, message, defaultValue);
        var window = new TextPromptWindow(viewModel)
        {
            Owner = System.Windows.Application.Current?.MainWindow
        };

        return window.ShowDialog() == true && viewModel.DialogResult == true
            ? viewModel.Value
            : null;
    }
}
