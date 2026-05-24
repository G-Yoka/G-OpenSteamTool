using System.Windows;
using CommunityToolkit.Mvvm.Messaging;
using OpenSteamTool.Manager.Helpers;
using OpenSteamTool.Manager.ViewModels;

namespace OpenSteamTool.Manager.Views;

public partial class TextPromptWindow : Window
{
    public TextPromptWindow(TextPromptViewModel viewModel)
    {
        InitializeComponent();
        DataContext = viewModel;
        Loaded += OnLoaded;
        Closed += OnClosed;
        WeakReferenceMessenger.Default.Register<DialogCloseRequestedMessage>(this, OnCloseRequested);
    }

    private void OnLoaded(object sender, RoutedEventArgs e)
    {
        PromptInput.Focus();
        PromptInput.SelectAll();
    }

    private void OnClosed(object? sender, EventArgs e)
        => WeakReferenceMessenger.Default.UnregisterAll(this);

    private void OnCloseRequested(object recipient, DialogCloseRequestedMessage message)
    {
        DialogResult = message.DialogResult;
        Close();
    }
}
