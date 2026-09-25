using Bloxstrap.Models.APIs;

namespace Bloxstrap.Utility
{
    public static class WeaoClient
    {
        private const string ExploitsEndpoint = "https://weao.gg/api/status/exploits";
        private const string UserAgent = "FishstrapInstaller";

        public readonly record struct WeaoResult(IReadOnlyList<WeaoExploit> Exploits, string? Error)
        {
            public bool Success => Error is null;
        }

        public static async Task<WeaoResult> GetExecutorsAsync(CancellationToken token = default)
        {
            const string LOG_IDENT = "WeaoClient::GetExecutorsAsync";

            try
            {
                using var request = new HttpRequestMessage(HttpMethod.Get, ExploitsEndpoint);
                request.Headers.UserAgent.ParseAdd(UserAgent);

                using var response = await App.HttpClient.SendAsync(request, token);
                if (!response.IsSuccessStatusCode)
                    return new WeaoResult(Array.Empty<WeaoExploit>(), $"weao returned HTTP {(int)response.StatusCode}");

                await using var stream = await response.Content.ReadAsStreamAsync(token);
                var all = await JsonSerializer.DeserializeAsync<List<WeaoExploit>>(
                    stream,
                    new JsonSerializerOptions { PropertyNameCaseInsensitive = true },
                    token);

                if (all is null)
                    return new WeaoResult(Array.Empty<WeaoExploit>(), "weao returned an empty response");

                var filtered = all
                    .Where(exploit => !exploit.Hidden
                        && string.Equals(exploit.Platform, "Windows", StringComparison.OrdinalIgnoreCase)
                        && VersionGuidValidator.IsValid(exploit.RbxVersion))
                    .OrderBy(exploit => exploit.Title, StringComparer.OrdinalIgnoreCase)
                    .ToArray();

                return new WeaoResult(filtered, null);
            }
            catch (Exception ex)
            {
                App.Logger.WriteException(LOG_IDENT, ex);
                return new WeaoResult(Array.Empty<WeaoExploit>(), $"couldn't load executor list ({ex.GetType().Name})");
            }
        }
    }
}
