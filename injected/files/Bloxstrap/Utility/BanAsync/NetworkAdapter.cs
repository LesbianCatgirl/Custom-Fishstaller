using System.Net.NetworkInformation;

namespace Bloxstrap.Utility.BanAsync
{
    public class NetworkAdapter
    {
        public string Id { get; set; } = "";
        public string Name { get; set; } = "";
        public string Description { get; set; } = "";
        public string PhysicalAddress { get; set; } = "";
        public NetworkInterfaceType InterfaceType { get; set; }
        public OperationalStatus Status { get; set; }
        public string ClassRegistryPath { get; set; } = "";

        public string DisplayLine => $"{Name} - {Description} ({FormatMac(PhysicalAddress)})";

        public static string FormatMac(string raw)
        {
            if (string.IsNullOrEmpty(raw))
                return "??-??-??-??-??-??";

            if (raw.Length != 12)
                return raw;

            return string.Join("-", Enumerable.Range(0, 6).Select(i => raw.Substring(i * 2, 2)));
        }
    }
}
