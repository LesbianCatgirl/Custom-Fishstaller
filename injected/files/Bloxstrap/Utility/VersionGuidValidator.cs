namespace Bloxstrap.Utility
{
    public static class VersionGuidValidator
    {
        private static readonly Regex GuidPattern = new(
            @"^version-[a-f0-9]{16}$",
            RegexOptions.Compiled | RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);

        public static bool IsValid(string? guid) =>
            !string.IsNullOrWhiteSpace(guid) && GuidPattern.IsMatch(guid);
    }
}
