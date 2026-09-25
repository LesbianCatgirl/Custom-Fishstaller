using System.Net.NetworkInformation;
using System.Security;
using Microsoft.Win32;

namespace Bloxstrap.Utility.BanAsync
{
    public static class MacSpoofer
    {
        private const string LOG_IDENT = "MacSpoofer";
        private const string NetworkClassKeyPath =
            @"SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}";

        private static readonly string[] VirtualKeywords =
        {
            "vpn", "tailscale", "wireguard", "openvpn", "tap", "teredo", "isatap",
            "miniport", "virtual", "hyper-v", "vmware", "virtualbox", "loopback", "wsl"
        };

        public static IReadOnlyList<NetworkAdapter> GetAdapters()
        {
            var result = new List<NetworkAdapter>();

            try
            {
                using var classKey = Registry.LocalMachine.OpenSubKey(NetworkClassKeyPath, writable: false);
                string[] subkeyNames = classKey?.GetSubKeyNames() ?? Array.Empty<string>();

                foreach (var nic in NetworkInterface.GetAllNetworkInterfaces())
                {
                    if (nic.NetworkInterfaceType != NetworkInterfaceType.Ethernet &&
                        nic.NetworkInterfaceType != NetworkInterfaceType.Wireless80211 &&
                        nic.NetworkInterfaceType != NetworkInterfaceType.GigabitEthernet)
                        continue;

                    if (IsVirtual(nic.Description) || IsVirtual(nic.Name))
                        continue;

                    string regPath = FindRegistryPath(classKey, subkeyNames, nic.Id);
                    if (string.IsNullOrEmpty(regPath))
                        continue;

                    result.Add(new NetworkAdapter
                    {
                        Id = nic.Id,
                        Name = nic.Name,
                        Description = nic.Description,
                        PhysicalAddress = nic.GetPhysicalAddress().ToString().ToUpperInvariant(),
                        InterfaceType = nic.NetworkInterfaceType,
                        Status = nic.OperationalStatus,
                        ClassRegistryPath = regPath
                    });
                }
            }
            catch (Exception ex)
            {
                App.Logger.WriteException(LOG_IDENT + "::GetAdapters", ex);
            }

            return result;
        }

        public static bool Spoof(NetworkAdapter adapter, string newMac, Action<string> log)
        {
            if (!IsValidMac(newMac))
            {
                log($"invalid MAC '{newMac}'");
                return false;
            }

            try
            {
                using var key = Registry.LocalMachine.OpenSubKey(adapter.ClassRegistryPath, writable: true);
                if (key is null)
                {
                    log($"registry path missing for {adapter.Name}");
                    return false;
                }

                key.SetValue("NetworkAddress", NormalizeMac(newMac), RegistryValueKind.String);
                log($"set NetworkAddress={NetworkAdapter.FormatMac(NormalizeMac(newMac))} on {adapter.Name}");
            }
            catch (SecurityException)
            {
                log($"access denied for {adapter.Name}. run as admin");
                return false;
            }
            catch (UnauthorizedAccessException)
            {
                log($"access denied for {adapter.Name}. run as admin");
                return false;
            }
            catch (Exception ex)
            {
                App.Logger.WriteException(LOG_IDENT + "::Spoof", ex);
                log($"couldn't set MAC for {adapter.Name}: {ex.Message}");
                return false;
            }

            return Restart(adapter.Name, log);
        }

        public static bool Revert(NetworkAdapter adapter, Action<string> log)
        {
            try
            {
                using var key = Registry.LocalMachine.OpenSubKey(adapter.ClassRegistryPath, writable: true);
                if (key is null)
                {
                    log($"registry path missing for {adapter.Name}");
                    return false;
                }

                key.DeleteValue("NetworkAddress", throwOnMissingValue: false);
                log($"cleared NetworkAddress for {adapter.Name}");
            }
            catch (SecurityException)
            {
                log($"access denied reverting {adapter.Name}. run as admin");
                return false;
            }
            catch (UnauthorizedAccessException)
            {
                log($"access denied reverting {adapter.Name}. run as admin");
                return false;
            }
            catch (Exception ex)
            {
                App.Logger.WriteException(LOG_IDENT + "::Revert", ex);
                log($"couldn't revert {adapter.Name}: {ex.Message}");
                return false;
            }

            return Restart(adapter.Name, log);
        }

        public static void ClearNetworkAddress(string adapterGuid)
        {
            try
            {
                using var classKey = Registry.LocalMachine.OpenSubKey(NetworkClassKeyPath, writable: false);
                if (classKey is null)
                    return;

                foreach (string sub in classKey.GetSubKeyNames())
                {
                    if (sub.Length != 4 || !int.TryParse(sub, out _))
                        continue;

                    using var subKey = classKey.OpenSubKey(sub, writable: true);
                    if (subKey?.GetValue("NetCfgInstanceId") is string id &&
                        string.Equals(id, adapterGuid, StringComparison.OrdinalIgnoreCase))
                    {
                        subKey.DeleteValue("NetworkAddress", throwOnMissingValue: false);
                        return;
                    }
                }
            }
            catch (Exception ex)
            {
                App.Logger.WriteException(LOG_IDENT + "::ClearNetworkAddress", ex);
            }
        }

        public static string RandomMac(string? ouiToMirror = null)
        {
            var bytes = new byte[6];
            System.Security.Cryptography.RandomNumberGenerator.Fill(bytes);

            if (!string.IsNullOrEmpty(ouiToMirror) && ouiToMirror.Length >= 6)
            {
                bytes[0] = Convert.ToByte(ouiToMirror.Substring(0, 2), 16);
                bytes[1] = Convert.ToByte(ouiToMirror.Substring(2, 2), 16);
                bytes[2] = Convert.ToByte(ouiToMirror.Substring(4, 2), 16);
            }
            else
            {
                bytes[0] = (byte)((bytes[0] & 0xFC) | 0x02);
            }

            return BitConverter.ToString(bytes).Replace("-", "").ToUpperInvariant();
        }

        public static bool IsValidMac(string mac)
        {
            string clean = NormalizeMac(mac);
            return clean.Length == 12 && Regex.IsMatch(clean, "^[0-9A-F]{12}$");
        }

        public static string NormalizeMac(string mac) =>
            mac.Replace("-", "").Replace(":", "").Replace(" ", "").ToUpperInvariant();

        private static bool Restart(string friendlyName, Action<string> log)
        {
            log($"restarting adapter '{friendlyName}'");
            bool down = Run("netsh", $"interface set interface name=\"{friendlyName}\" admin=disabled", log);
            Thread.Sleep(500);
            bool up = Run("netsh", $"interface set interface name=\"{friendlyName}\" admin=enabled", log);
            return down && up;
        }

        private static bool IsVirtual(string text)
        {
            string lower = text.ToLowerInvariant();
            return VirtualKeywords.Any(lower.Contains);
        }

        private static string FindRegistryPath(RegistryKey? classKey, string[] subkeyNames, string adapterGuid)
        {
            if (classKey is null)
                return "";

            foreach (string sub in subkeyNames)
            {
                if (sub.Length != 4 || !int.TryParse(sub, out _))
                    continue;

                try
                {
                    using var subKey = classKey.OpenSubKey(sub, writable: false);
                    if (subKey?.GetValue("NetCfgInstanceId") is string id &&
                        string.Equals(id, adapterGuid, StringComparison.OrdinalIgnoreCase))
                        return $"{NetworkClassKeyPath}\\{sub}";
                }
                catch (Exception ex)
                {
                    App.Logger.WriteException(LOG_IDENT + "::SubKeyScan", ex);
                }
            }

            return "";
        }

        private static bool Run(string fileName, string args, Action<string> log)
        {
            try
            {
                var startInfo = new ProcessStartInfo
                {
                    FileName = fileName,
                    Arguments = args,
                    UseShellExecute = false,
                    CreateNoWindow = true,
                    RedirectStandardOutput = true,
                    RedirectStandardError = true
                };

                using var process = Process.Start(startInfo);
                if (process is null)
                    return false;

                if (!process.WaitForExit(15000))
                {
                    process.Kill();
                    log($"{fileName} timed out");
                    return false;
                }

                if (process.ExitCode != 0)
                {
                    string error = process.StandardError.ReadToEnd().Trim();
                    log($"{fileName} exited {process.ExitCode}: {error}");
                    return false;
                }

                return true;
            }
            catch (Exception ex)
            {
                App.Logger.WriteException(LOG_IDENT + "::Run", ex);
                log($"couldn't run {fileName}: {ex.Message}");
                return false;
            }
        }
    }
}
