namespace OpenSteamTool.Manager.Helpers;

public interface ISecretPromptService
{
    string? Show(string title, string message);
}
