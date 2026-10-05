namespace Bloxstrap
{
    internal static class MultiInstanceWatcher
    {
        private const string MutexName = "ROBLOX_singletonMutex";
        private const string InitEventName = "Bloxstrap-MultiInstanceWatcherInitialisationFinished";
        private static int GetOpenProcessesCount()
        {
            const string LOG_IDENT = "MultiInstanceWatcher::GetOpenProcessesCount";
            try
            {
                int count = Process.GetProcesses().Count(x => x.ProcessName is "RobloxPlayerBeta" or "Fishstrap");
                return count - 1;
            }
            catch (Exception ex)
            {
                App.Logger.WriteException(LOG_IDENT, ex);
                return -1;
            }
        }
        private static void FireInitialisedEvent()
        {
            using EventWaitHandle initEventHandle = new EventWaitHandle(false, EventResetMode.AutoReset, InitEventName);
            initEventHandle.Set();
        }
        public static void Launch()
        {
            const string LOG_IDENT = "MultiInstanceWatcher::Launch";
            if (Utilities.DoesMutexExist(MutexName))
            {
                App.Logger.WriteLine(LOG_IDENT, "Roblox singleton mutex already exists");
                return;
            }
            using EventWaitHandle initEventHandle = new EventWaitHandle(false, EventResetMode.AutoReset, InitEventName);
            Process.Start(Paths.Process, "-multiinstancewatcher");
            bool initSuccess = initEventHandle.WaitOne(TimeSpan.FromSeconds(2));
            App.Logger.WriteLine(LOG_IDENT, initSuccess ? "Initialisation finished signalled, continuing." : "Did not receive the initialisation finished signal, continuing.");
        }
        public static void Start()
        {
            const string LOG_IDENT = "MultiInstanceWatcher::Start";
            App.Logger.WriteLine(LOG_IDENT, "Starting multi-instance watcher");
            Task.Run(Run).ContinueWith(t =>
            {
                App.Logger.WriteLine(LOG_IDENT, "Multi instance watcher task has finished");
                if (t.IsFaulted && t.Exception is not null)
                    App.FinalizeExceptionHandling(t.Exception);
                App.Terminate();
            });
        }
        public static void Run()
        {
            const string LOG_IDENT = "MultiInstanceWatcher::Run";
            bool acquiredMutex;
            using Mutex mutex = new Mutex(false, MutexName);
            try
            {
                acquiredMutex = mutex.WaitOne(0);
            }
            catch (AbandonedMutexException)
            {
                acquiredMutex = true;
            }
            if (!acquiredMutex)
            {
                App.Logger.WriteLine(LOG_IDENT, "Client singleton mutex is already acquired");
                FireInitialisedEvent();
                return;
            }
            App.Logger.WriteLine(LOG_IDENT, "Acquired mutex!");
            FireInitialisedEvent();
            int count;
            do
            {
                Thread.Sleep(5000);
                count = GetOpenProcessesCount();
            }
            while (count == -1 || count > 0);
            App.Logger.WriteLine(LOG_IDENT, "All Roblox related processes have closed, exiting!");
        }
    }
}
