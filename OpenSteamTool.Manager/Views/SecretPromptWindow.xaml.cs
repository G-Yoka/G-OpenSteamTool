using System.Windows;

namespace OpenSteamTool.Manager.Views;

public partial class SecretPromptWindow : Window
{
    public SecretPromptWindow(string title, string message)
    {
        InitializeComponent();
        Title = title;
        PromptMessage.Text = message;
    }

    public string? Value { get; private set; }

    private void OnConfirmClick(object sender, RoutedEventArgs e)
    {
        Value = SecretInput.Password;
        DialogResult = true;
    }

    private void OnCancelClick(object sender, RoutedEventArgs e)
    {
        Value = null;
        DialogResult = false;
    }
}
