using System.Collections.ObjectModel;
using System.Security.Principal;
using System.Windows;
using System.Windows.Input;
using Bloxstrap.Utility.BanAsync;
using CommunityToolkit.Mvvm.Input;

namespace Bloxstrap.UI.ViewModels.Settings
{
    public class BanAsyncViewModel : NotifyPropertyChangedViewModel
    {
        private const string LOG_IDENT = "BanAsyncViewModel";

        public BanAsyncViewModel()
        {
            IsElevated = IsAdmin();
            RefreshAdapters();
        }

        public bool IsElevated { get; }
        public Visibility ElevationWarningVisibility => IsElevated ? Visibility.Collapsed : Visibility.Visible;
        public Visibility AdminFeaturesVisibility => IsElevated ? Visibility.Visible : Visibility.Collapsed;

        public ObservableCollection<NetworkAdapter> Adapters { get; } = new();
        public ObservableCollection<string> ActivityLog { get; } = new();

        private NetworkAdapter? _selectedAdapter;
        public NetworkAdapter? SelectedAdapter
        {
            get => _selectedAdapter;
            set
            {
                _selectedAdapter = value;
                OnPropertyChanged(nameof(SelectedAdapter));
                OnPropertyChanged(nameof(CurrentMacFormatted));
                OnPropertyChanged(nameof(OriginalMacFormatted));
            }
        }

        public string CurrentMacFormatted => SelectedAdapter is null ? "(no adapter selected)" : NetworkAdapter.FormatMac(SelectedAdapter.PhysicalAddress);

        public string OriginalMacFormatted => SelectedAdapter is not null &&
            App.Settings.Prop.BanAsyncOriginalMacByGuid.TryGetValue(SelectedAdapter.Id, out var original)
                ? NetworkAdapter.FormatMac(original)
                : "(none saved yet)";

        public bool PreserveInGameSettings
        {
            get => App.Settings.Prop.BanAsyncPreserveInGameSettings;
            set { App.Settings.Prop.BanAsyncPreserveInGameSettings = value; OnPropertyChanged(nameof(PreserveInGameSettings)); }
        }

        public bool PreserveFastFlags
        {
            get => App.Settings.Prop.BanAsyncPreserveFastFlags;
            set { App.Settings.Prop.BanAsyncPreserveFastFlags = value; OnPropertyChanged(nameof(PreserveFastFlags)); }
        }

        public bool IncludeStudioFolders
        {
            get => App.Settings.Prop.BanAsyncIncludeStudioFolders;
            set { App.Settings.Prop.BanAsyncIncludeStudioFolders = value; OnPropertyChanged(nameof(IncludeStudioFolders)); }
        }

        public bool CleanFishstrapVersions
        {
            get => App.Settings.Prop.BanAsyncCleanVersions;
            set { App.Settings.Prop.BanAsyncCleanVersions = value; OnPropertyChanged(nameof(CleanFishstrapVersions)); }
        }

        public bool Persistent
        {
            get => App.Settings.Prop.BanAsyncPersistent;
            set { App.Settings.Prop.BanAsyncPersistent = value; OnPropertyChanged(nameof(Persistent)); }
        }

        public bool OuiMirror
        {
            get => App.Settings.Prop.BanAsyncOuiMirror;
            set { App.Settings.Prop.BanAsyncOuiMirror = value; OnPropertyChanged(nameof(OuiMirror)); }
        }

        public bool MachineGuidAcknowledged
        {
            get => App.Settings.Prop.BanAsyncMachineGuidAcknowledged;
            set
            {
                App.Settings.Prop.BanAsyncMachineGuidAcknowledged = value;
                OnPropertyChanged(nameof(MachineGuidAcknowledged));
                OnPropertyChanged(nameof(MachineGuidActionsEnabled));
            }
        }

        public bool MachineGuidActionsEnabled => IsElevated && MachineGuidAcknowledged;

        private string _customMac = "";
        public string CustomMac
        {
            get => _customMac;
            set { _customMac = value ?? ""; OnPropertyChanged(nameof(CustomMac)); }
        }

        public string CurrentMachineGuid => MachineGuidSpoofer.Read() ?? "(unreadable)";

        public string MachineGuidBackupText => string.IsNullOrEmpty(App.Settings.Prop.BanAsyncOriginalMachineGuid)
            ? "no original MachineGuid backed up yet"
            : $"original MachineGuid: {App.Settings.Prop.BanAsyncOriginalMachineGuid}";

        public ICommand RelaunchAdminCommand => new RelayCommand(RelaunchAdmin);
        public ICommand RefreshAdaptersCommand => new RelayCommand(RefreshAdapters);
        public ICommand CleanTracesCommand => new AsyncRelayCommand(CleanAsync);
        public ICommand SpoofCommand => new AsyncRelayCommand(SpoofAsync);
        public ICommand RevertCommand => new AsyncRelayCommand(RevertAsync);
        public ICommand ShuffleMacCommand => new RelayCommand(ShuffleMac);
        public ICommand RandomizeMachineGuidCommand => new AsyncRelayCommand(RandomizeGuidAsync);
        public ICommand RestoreMachineGuidCommand => new AsyncRelayCommand(RestoreGuidAsync);
        public ICommand ClearLogCommand => new RelayCommand(() => ActivityLog.Clear());

        private void RefreshAdapters()
        {
            string? previousId = SelectedAdapter?.Id;
            Adapters.Clear();

            foreach (var adapter in MacSpoofer.GetAdapters())
                Adapters.Add(adapter);

            SelectedAdapter = Adapters.FirstOrDefault(adapter => adapter.Id == previousId) ?? Adapters.FirstOrDefault();
            AddLog($"found {Adapters.Count} physical adapter(s)");
        }

        private async Task CleanAsync()
        {
            var confirm = Frontend.ShowMessageBox(
                "this closes Roblox and deletes local caches, logs, temp folders, prefetch entries, and HKCU Roblox registry data. Fishstrap settings stay untouched.\n\ncontinue?",
                MessageBoxImage.Warning,
                MessageBoxButton.YesNo,
                MessageBoxResult.No);

            if (confirm != MessageBoxResult.Yes)
                return;

            AddLog("starting trace cleanup");
            var options = new CleanupEngine.CleanupOptions
            {
                PreserveInGameSettings = PreserveInGameSettings,
                PreserveFastFlags = PreserveFastFlags,
                IncludeStudioFolders = IncludeStudioFolders,
                CleanFishstrapVersions = CleanFishstrapVersions
            };

            var result = await Task.Run(() => CleanupEngine.Clean(options, AddLog));
            AddLog($"cleanup done. removed {result.DeletedDirectories} dir(s), {result.DeletedFiles} file(s), {result.RegistryKeysRemoved} registry key(s), kept {result.PreservedFiles} file(s), skipped {result.Skipped.Count}");
        }

        private async Task SpoofAsync()
        {
            if (!IsElevated)
            {
                AddLog("spoofing needs admin permissions");
                return;
            }

            var adapter = SelectedAdapter;
            if (adapter is null)
            {
                AddLog("no adapter selected");
                return;
            }

            string mac = string.IsNullOrWhiteSpace(CustomMac)
                ? MacSpoofer.RandomMac(OuiMirror ? adapter.PhysicalAddress : null)
                : MacSpoofer.NormalizeMac(CustomMac);

            if (!MacSpoofer.IsValidMac(mac))
            {
                Frontend.ShowMessageBox("that MAC address is invalid. use 12 hex characters", MessageBoxImage.Warning);
                return;
            }

            if (!App.Settings.Prop.BanAsyncOriginalMacByGuid.ContainsKey(adapter.Id))
                App.Settings.Prop.BanAsyncOriginalMacByGuid[adapter.Id] = adapter.PhysicalAddress;

            bool ok = await Task.Run(() => MacSpoofer.Spoof(adapter, mac, AddLog));
            if (ok && !App.Settings.Prop.BanAsyncSpoofedAdapterGuids.Contains(adapter.Id))
                App.Settings.Prop.BanAsyncSpoofedAdapterGuids.Add(adapter.Id);

            RefreshAdapters();
        }

        private async Task RevertAsync()
        {
            if (!IsElevated)
            {
                AddLog("reverting needs admin permissions");
                return;
            }

            var adapter = SelectedAdapter;
            if (adapter is null)
            {
                AddLog("no adapter selected");
                return;
            }

            bool ok = await Task.Run(() => MacSpoofer.Revert(adapter, AddLog));
            if (ok)
            {
                App.Settings.Prop.BanAsyncSpoofedAdapterGuids.Remove(adapter.Id);
                App.Settings.Prop.BanAsyncOriginalMacByGuid.Remove(adapter.Id);
            }

            RefreshAdapters();
        }

        private void ShuffleMac()
        {
            string seed = OuiMirror && SelectedAdapter is not null ? SelectedAdapter.PhysicalAddress : null!;
            CustomMac = NetworkAdapter.FormatMac(MacSpoofer.RandomMac(seed));
        }

        private async Task RandomizeGuidAsync()
        {
            if (!MachineGuidActionsEnabled)
            {
                AddLog("MachineGuid changes need admin permissions and acknowledgement");
                return;
            }

            await Task.Run(() =>
            {
                string? current = MachineGuidSpoofer.Read();
                if (!string.IsNullOrEmpty(current) && string.IsNullOrEmpty(App.Settings.Prop.BanAsyncOriginalMachineGuid))
                    App.Settings.Prop.BanAsyncOriginalMachineGuid = current;

                MachineGuidSpoofer.Set(MachineGuidSpoofer.Random(), AddLog);
            });

            OnPropertyChanged(nameof(CurrentMachineGuid));
            OnPropertyChanged(nameof(MachineGuidBackupText));
        }

        private async Task RestoreGuidAsync()
        {
            if (!IsElevated)
            {
                AddLog("restoring MachineGuid needs admin permissions");
                return;
            }

            string original = App.Settings.Prop.BanAsyncOriginalMachineGuid;
            if (string.IsNullOrEmpty(original))
            {
                Frontend.ShowMessageBox("no original MachineGuid is saved", MessageBoxImage.Information);
                return;
            }

            bool ok = await Task.Run(() => MachineGuidSpoofer.Set(original, AddLog));
            if (ok)
                App.Settings.Prop.BanAsyncOriginalMachineGuid = "";

            OnPropertyChanged(nameof(CurrentMachineGuid));
            OnPropertyChanged(nameof(MachineGuidBackupText));
        }

        private static bool IsAdmin()
        {
            try
            {
                using var identity = WindowsIdentity.GetCurrent();
                return new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator);
            }
            catch
            {
                return false;
            }
        }

        private void RelaunchAdmin()
        {
            try
            {
                Process.Start(new ProcessStartInfo
                {
                    FileName = Paths.Process,
                    UseShellExecute = true,
                    Verb = "runas",
                    Arguments = "-menu"
                });
                App.Terminate();
            }
            catch (Exception ex)
            {
                App.Logger.WriteException(LOG_IDENT + "::RelaunchAdmin", ex);
                Frontend.ShowMessageBox($"couldn't relaunch as admin.\n\n{ex.Message}", MessageBoxImage.Warning);
            }
        }

        private void AddLog(string line)
        {
            string stamped = $"[{DateTime.Now:HH:mm:ss}] {line}";
            App.Logger.WriteLine(LOG_IDENT, line);

            void apply()
            {
                ActivityLog.Add(stamped);
                while (ActivityLog.Count > 500)
                    ActivityLog.RemoveAt(0);
            }

            if (Application.Current?.Dispatcher is { } dispatcher && !dispatcher.CheckAccess())
                dispatcher.BeginInvoke(new Action(apply));
            else
                apply();
        }
    }
}
