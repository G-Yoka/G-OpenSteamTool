namespace OpenSteamTool.Manager.Helpers;

public interface ITextPromptService
{
    string? Show(string title, string message, string defaultValue);
}
