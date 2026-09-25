using Microsoft.Win32;

namespace Bloxstrap.Utility.BanAsync
{
    public static class CleanupEngine
    {
        private const string LOG_IDENT = "CleanupEngine";

        private static readonly string[] RobloxProcessNames =
        {
            "RobloxPlayerBeta",
            "RobloxStudioBeta",
            "RobloxCrashHandler",
            "RobloxPlayerLauncher",
            "RobloxPlayerInstaller",
            "Roblox"
        };

        public class CleanupOptions
        {
            public bool PreserveInGameSettings { get; set; } = true;
            public bool PreserveFastFlags { get; set; } = true;
            public bool IncludeStudioFolders { get; set; }
            public bool CleanFishstrapVersions { get; set; }
        }

        public class CleanupResult
        {
            public int DeletedDirectories { get; set; }
            public int DeletedFiles { get; set; }
            public int RegistryKeysRemoved { get; set; }
            public int PreservedFiles { get; set; }
            public List<string> Skipped { get; } = new();
        }

        public static CleanupResult Clean(CleanupOptions options, Action<string> log)
        {
            var result = new CleanupResult();

            log("closing Roblox processes");
            CloseRoblox(log);

            Dictionary<string, byte[]> preserveBackup = SavePreservedFiles(options, log);
            result.PreservedFiles = preserveBackup.Count;

            foreach (string path in GetTargets(options))
            {
                if (!Directory.Exists(path))
                    continue;

                DeleteDirectory(path, result, log);
            }

            foreach (string dir in GetDirectories(Path.GetTempPath(), "Roblox*"))
                DeleteDirectory(dir, result, log);

            result.DeletedFiles += CleanPrefetch(log);

            try
            {
                Registry.CurrentUser.DeleteSubKeyTree(@"Software\ROBLOX Corporation", throwOnMissingSubKey: false);
                result.RegistryKeysRemoved++;
                log(@"removed HKCU\Software\ROBLOX Corporation");
            }
            catch (Exception ex)
            {
                App.Logger.WriteException(LOG_IDENT + "::RegistryHKCU", ex);
                log($"couldn't remove Roblox HKCU registry data: {ex.Message}");
            }

            foreach (var (path, bytes) in preserveBackup)
            {
                try
                {
                    Directory.CreateDirectory(Path.GetDirectoryName(path)!);
                    File.WriteAllBytes(path, bytes);
                    log($"restored {path}");
                }
                catch (Exception ex)
                {
                    App.Logger.WriteException(LOG_IDENT + "::Restore", ex);
                    log($"couldn't restore {path}: {ex.Message}");
                }
            }

            return result;
        }

        private static void CloseRoblox(Action<string> log)
        {
            foreach (string name in RobloxProcessNames)
            {
                foreach (var process in Process.GetProcessesByName(name))
                {
                    try
                    {
                        process.Kill();
                        process.WaitForExit(2000);
                        log($"closed {name} ({process.Id})");
                    }
                    catch (Exception ex)
                    {
                        App.Logger.WriteException(LOG_IDENT + "::Kill", ex);
                        log($"couldn't close {name} ({process.Id}): {ex.Message}");
                    }
                    finally
                    {
                        process.Dispose();
                    }
                }
            }
        }

        private static IEnumerable<string> GetTargets(CleanupOptions options)
        {
            string localAppData = Paths.LocalAppData;
            string appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
            string programData = Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData);

            yield return Path.Combine(localAppData, "Roblox");
            yield return Path.Combine(appData, "Roblox", "logs");
            yield return Path.Combine(appData, "Roblox", "http");
            yield return Path.Combine(programData, "Roblox");

            if (options.IncludeStudioFolders)
            {
                yield return Path.Combine(localAppData, "Roblox", "Studio");
                yield return Path.Combine(appData, "Roblox", "Studio");
            }

            if (options.CleanFishstrapVersions)
                yield return Paths.Versions;
        }

        private static Dictionary<string, byte[]> SavePreservedFiles(CleanupOptions options, Action<string> log)
        {
            var snapshot = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
            string robloxRoot = Path.Combine(Paths.LocalAppData, "Roblox");

            if (!Directory.Exists(robloxRoot))
                return snapshot;

            if (options.PreserveInGameSettings)
                SaveMatching(robloxRoot, "GlobalBasicSettings_*.xml", snapshot, log);

            if (options.PreserveFastFlags)
            {
                SaveMatching(robloxRoot, "ClientAppSettings.json", snapshot, log);
                SaveMatching(robloxRoot, "ClientSettings.json", snapshot, log);
            }

            return snapshot;
        }

        private static void SaveMatching(string root, string pattern, Dictionary<string, byte[]> snapshot, Action<string> log)
        {
            foreach (string file in GetFiles(root, pattern))
            {
                try
                {
                    var info = new FileInfo(file);
                    if (info.Length > 16L * 1024 * 1024)
                        continue;

                    snapshot[file] = File.ReadAllBytes(file);
                }
                catch (Exception ex)
                {
                    App.Logger.WriteException(LOG_IDENT + "::Capture", ex);
                    log($"couldn't keep {file}: {ex.Message}");
                }
            }
        }

        private static void DeleteDirectory(string path, CleanupResult result, Action<string> log)
        {
            try
            {
                Directory.Delete(path, recursive: true);
                result.DeletedDirectories++;
                log($"deleted directory {path}");
            }
            catch (Exception ex)
            {
                App.Logger.WriteException(LOG_IDENT + "::DeleteDir", ex);
                result.Skipped.Add(path);
                log($"skipped {path}: {ex.Message}");
            }
        }

        private static int CleanPrefetch(Action<string> log)
        {
            int count = 0;
            string prefetchDir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Windows), "Prefetch");

            foreach (string pattern in new[] { "ROBLOXCRASHHANDLER.EXE-*.pf", "ROBLOXPLAYERBETA.EXE-*.pf" })
            {
                foreach (string file in GetFiles(prefetchDir, pattern))
                {
                    try
                    {
                        File.Delete(file);
                        count++;
                        log($"deleted prefetch {Path.GetFileName(file)}");
                    }
                    catch (UnauthorizedAccessException)
                    {
                        log("skipped prefetch entry because admin permissions are needed");
                    }
                    catch (Exception ex)
                    {
                        App.Logger.WriteException(LOG_IDENT + "::Prefetch", ex);
                        log($"couldn't delete prefetch {Path.GetFileName(file)}: {ex.Message}");
                    }
                }
            }

            return count;
        }

        private static IEnumerable<string> GetDirectories(string root, string pattern)
        {
            try
            {
                return Directory.Exists(root) ? Directory.EnumerateDirectories(root, pattern, SearchOption.TopDirectoryOnly).ToArray() : Array.Empty<string>();
            }
            catch
            {
                return Array.Empty<string>();
            }
        }

        private static IEnumerable<string> GetFiles(string root, string pattern)
        {
            try
            {
                return Directory.Exists(root) ? Directory.EnumerateFiles(root, pattern, SearchOption.AllDirectories).ToArray() : Array.Empty<string>();
            }
            catch
            {
                return Array.Empty<string>();
            }
        }
    }
}
