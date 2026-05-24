using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using CommunityToolkit.Mvvm.Messaging;
using OpenSteamTool.Manager.Helpers;

namespace OpenSteamTool.Manager.ViewModels;

public partial class TextPromptViewModel : ViewModelBase
{
    [ObservableProperty]
    private string title;

    [ObservableProperty]
    private string message;

    [ObservableProperty]
    private string value;

    [ObservableProperty]
    private bool? dialogResult;

    public TextPromptViewModel(string title, string message, string defaultValue)
    {
        Title = title;
        Message = message;
        Value = defaultValue;
    }

    [RelayCommand]
    private void Confirm()
    {
        DialogResult = true;
        WeakReferenceMessenger.Default.Send(new DialogCloseRequestedMessage(true));
    }

    [RelayCommand]
    private void Cancel()
    {
        DialogResult = false;
        WeakReferenceMessenger.Default.Send(new DialogCloseRequestedMessage(false));
    }
}
