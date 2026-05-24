using CommunityToolkit.Mvvm.ComponentModel;

namespace OpenSteamTool.Manager.ViewModels;

public abstract partial class ViewModelBase : ObservableObject
{
    [ObservableProperty]
    private bool isBusy;

    [ObservableProperty]
    private string busyMessage = string.Empty;
}
