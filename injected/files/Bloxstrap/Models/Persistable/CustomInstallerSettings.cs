using System.Collections.ObjectModel;

namespace Bloxstrap.Models.Persistable
{
    public partial class Settings
    {
        public bool UseExecutors { get; set; } = false;
        public string ExecutorChannel { get; set; } = "";

        public bool BanAsyncPreserveInGameSettings { get; set; } = true;
        public bool BanAsyncPreserveFastFlags { get; set; } = true;
        public bool BanAsyncIncludeStudioFolders { get; set; } = false;
        public bool BanAsyncCleanVersions { get; set; } = false;
        public bool BanAsyncPersistent { get; set; } = true;
        public bool BanAsyncOuiMirror { get; set; } = true;
        public bool BanAsyncMachineGuidAcknowledged { get; set; } = false;
        public string BanAsyncOriginalMachineGuid { get; set; } = "";
        public ObservableCollection<string> BanAsyncSpoofedAdapterGuids { get; set; } = new();
        public Dictionary<string, string> BanAsyncOriginalMacByGuid { get; set; } = new();
    }
}
