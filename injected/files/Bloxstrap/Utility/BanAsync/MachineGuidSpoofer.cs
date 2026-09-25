using System.Security;
using Microsoft.Win32;

namespace Bloxstrap.Utility.BanAsync
{
    public static class MachineGuidSpoofer
    {
        private const string LOG_IDENT = "MachineGuidSpoofer";
        private const string KeyPath = @"SOFTWARE\Microsoft\Cryptography";
        private const string ValueName = "MachineGuid";

        public static string? Read()
        {
            try
            {
                using var key = Registry.LocalMachine.OpenSubKey(KeyPath, writable: false);
                return key?.GetValue(ValueName) as string;
            }
            catch (Exception ex)
            {
                App.Logger.WriteException(LOG_IDENT + "::Read", ex);
                return null;
            }
        }

        public static bool Set(string newGuid, Action<string> log)
        {
            if (!Guid.TryParse(newGuid, out _))
            {
                log($"invalid MachineGuid '{newGuid}'");
                return false;
            }

            try
            {
                using var key = Registry.LocalMachine.OpenSubKey(KeyPath, writable: true);
                if (key is null)
                {
                    log("couldn't open the MachineGuid registry key. run as admin");
                    return false;
                }

                key.SetValue(ValueName, newGuid, RegistryValueKind.String);
                log($"MachineGuid set to {newGuid}");
                return true;
            }
            catch (SecurityException)
            {
                log("access denied. run as admin");
                return false;
            }
            catch (UnauthorizedAccessException)
            {
                log("access denied. run as admin");
                return false;
            }
            catch (Exception ex)
            {
                App.Logger.WriteException(LOG_IDENT + "::Set", ex);
                log($"couldn't write MachineGuid: {ex.Message}");
                return false;
            }
        }

        public static string Random() => Guid.NewGuid().ToString();
    }
}
