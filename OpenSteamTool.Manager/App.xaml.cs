using System.Windows;
using OpenSteamTool.Manager.Helpers;
using OpenSteamTool.Manager.Views;

namespace OpenSteamTool.Manager;

public partial class App : System.Windows.Application
{
    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        ConfigureExceptionHandlers();

        var window = new MainWindow(AppServices.MainViewModel);
        MainWindow = window;
        window.Show();
    }

    private void ConfigureExceptionHandlers()
    {
        DispatcherUnhandledException += (_, args) =>
        {
            System.Windows.MessageBox.Show(args.Exception.Message, "OpenSteamTool 管理器", MessageBoxButton.OK, MessageBoxImage.Error);
            args.Handled = true;
        };

        AppDomain.CurrentDomain.UnhandledException += (_, args) =>
        {
            if (args.ExceptionObject is Exception ex)
            {
                System.Windows.MessageBox.Show(ex.Message, "OpenSteamTool 管理器", MessageBoxButton.OK, MessageBoxImage.Error);
            }
        };

        TaskScheduler.UnobservedTaskException += (_, args) =>
        {
            System.Windows.MessageBox.Show(args.Exception.Message, "OpenSteamTool 管理器", MessageBoxButton.OK, MessageBoxImage.Error);
            args.SetObserved();
        };
    }
}
