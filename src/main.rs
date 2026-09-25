use std::env;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_FISHSTRAP_REPO: &str = "https://github.com/returnrqt/fishstrap.git";
const APP_EXE: &str = "Fishstrap.exe";
const UPDATER_EXE: &str = "FishstrapInstaller.exe";
const INJECTED_FILES: &[(&str, &str)] = &[
    (
        "Bloxstrap/Models/APIs/WeaoExploit.cs",
        include_str!("../injected/files/Bloxstrap/Models/APIs/WeaoExploit.cs"),
    ),
    (
        "Bloxstrap/Models/Persistable/CustomInstallerSettings.cs",
        include_str!("../injected/files/Bloxstrap/Models/Persistable/CustomInstallerSettings.cs"),
    ),
    (
        "Bloxstrap/Utility/VersionGuidValidator.cs",
        include_str!("../injected/files/Bloxstrap/Utility/VersionGuidValidator.cs"),
    ),
    (
        "Bloxstrap/Utility/WeaoClient.cs",
        include_str!("../injected/files/Bloxstrap/Utility/WeaoClient.cs"),
    ),
    (
        "Bloxstrap/Utility/BanAsync/NetworkAdapter.cs",
        include_str!("../injected/files/Bloxstrap/Utility/BanAsync/NetworkAdapter.cs"),
    ),
    (
        "Bloxstrap/Utility/BanAsync/MachineGuidSpoofer.cs",
        include_str!("../injected/files/Bloxstrap/Utility/BanAsync/MachineGuidSpoofer.cs"),
    ),
    (
        "Bloxstrap/Utility/BanAsync/MacSpoofer.cs",
        include_str!("../injected/files/Bloxstrap/Utility/BanAsync/MacSpoofer.cs"),
    ),
    (
        "Bloxstrap/Utility/BanAsync/CleanupEngine.cs",
        include_str!("../injected/files/Bloxstrap/Utility/BanAsync/CleanupEngine.cs"),
    ),
    (
        "Bloxstrap/UI/ViewModels/Settings/BanAsyncViewModel.cs",
        include_str!("../injected/files/Bloxstrap/UI/ViewModels/Settings/BanAsyncViewModel.cs"),
    ),
    (
        "Bloxstrap/UI/Elements/Settings/Pages/BanAsyncPage.xaml",
        include_str!("../injected/files/Bloxstrap/UI/Elements/Settings/Pages/BanAsyncPage.xaml"),
    ),
    (
        "Bloxstrap/UI/Elements/Settings/Pages/BanAsyncPage.xaml.cs",
        include_str!("../injected/files/Bloxstrap/UI/Elements/Settings/Pages/BanAsyncPage.xaml.cs"),
    ),
];

#[derive(Debug)]
struct Config {
    fishstrap_repo: String,
    branch: Option<String>,
    install_dir: PathBuf,
    keep_temp: bool,
    build_only: bool,
    wait_pid: Option<u32>,
    launch_args: Vec<String>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("installer failed: {err}");
        if !env::args().any(|arg| arg == "--no-pause") {
            wait_for_enter();
        }
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = Config::from_args()?;
    ensure_tool("git", "Git.Git")?;
    let work_dir = env::temp_dir().join(format!("fishstrap-build-{}", timestamp()?));
    let fishstrap_dir = work_dir.join("fishstrap");
    fs::create_dir_all(&work_dir)?;
    println!("cloning Fishstrap into {}", fishstrap_dir.display());
    clone_repo(
        &config.fishstrap_repo,
        config.branch.as_deref(),
        &fishstrap_dir,
    )?;
    ensure_dotnet_sdk(&fishstrap_dir)?;
    println!("writing injected files");
    write_injected_files(&fishstrap_dir)?;
    println!("adding custom integrations");
    inject_source_changes(&fishstrap_dir)?;
    ensure_settings_compatibility(&fishstrap_dir)?;
    let artifact_dir = build_fishstrap(&fishstrap_dir, &work_dir)?;
    if config.build_only {
        println!("build done at {}", artifact_dir.display());
        println!("temp workspace at {}", work_dir.display());
        return Ok(());
    }
    close_running_apps()?;
    if let Some(pid) = config.wait_pid {
        wait_for_process_exit(pid)?;
    }
    fs::create_dir_all(&config.install_dir)?;
    copy_dir_contents(&artifact_dir, &config.install_dir)?;
    let updater_location = copy_updater_executable(&config.install_dir)?;
    println!("updater at {}", updater_location.display());
    create_start_menu_shortcut(&config.install_dir)?;
    register_install(&config.install_dir)?;
    if !config.launch_args.is_empty() {
        launch_installed_app(&config.install_dir, &config.launch_args)?;
    }
    if !config.keep_temp {
        let _ = fs::remove_dir_all(&work_dir);
    } else {
        println!("temp workspace at {}", work_dir.display());
    }
    println!("fishstrap installed to {}", config.install_dir.display());
    Ok(())
}

impl Config {
    fn from_args() -> Result<Self, Box<dyn Error>> {
        let mut fishstrap_repo =
            env::var("FISHSTRAP_REPO").unwrap_or_else(|_| DEFAULT_FISHSTRAP_REPO.to_string());
        let mut branch = env::var("FISHSTRAP_BRANCH").ok();
        let mut install_dir = default_install_dir()?;
        let mut keep_temp = false;
        let mut build_only = false;
        let mut wait_pid = None;
        let mut launch_args = Vec::new();
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--repo" => fishstrap_repo = required_value(&mut args, "--repo")?,
                "--branch" => branch = Some(required_value(&mut args, "--branch")?),
                "--out" | "--dir" => install_dir = PathBuf::from(required_value(&mut args, &arg)?),
                "--keep-temp" => keep_temp = true,
                "--build-only" => build_only = true,
                "--wait-pid" => wait_pid = Some(required_value(&mut args, "--wait-pid")?.parse()?),
                "--app-arg" => launch_args.push(required_value(&mut args, "--app-arg")?),
                "--no-pause" => {},
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => return Err(format!("unknown argument: {arg}").into()),
            }
        }
        Ok(Self {
            fishstrap_repo,
            branch,
            install_dir,
            keep_temp,
            build_only,
            wait_pid,
            launch_args,
        })
    }
}

fn required_value(
    args: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<String, Box<dyn Error>> {
    args.next()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} requires a value").into())
}

fn print_help() {
    println!("usage: install.exe [--repo <url>] [--branch <name>] [--out <dir>] [--keep-temp] [--build-only] [--wait-pid <pid>] [--app-arg <arg>] [--no-pause]");
    println!();
    println!("defaults:");
    println!("  --repo          {DEFAULT_FISHSTRAP_REPO}");
    println!("  --out           %LOCALAPPDATA%\\Fishstrap");
    println!();
    println!("env variables: FISHSTRAP_REPO, FISHSTRAP_BRANCH");
}

fn default_install_dir() -> Result<PathBuf, Box<dyn Error>> {
    let local_app_data = env::var_os("LOCALAPPDATA")
        .ok_or("LOCALAPPDATA isn't set somehow. pass --out <path> to pick an install folder")?;
    Ok(PathBuf::from(local_app_data).join("Fishstrap"))
}

fn clone_repo(repo: &str, branch: Option<&str>, destination: &Path) -> Result<(), Box<dyn Error>> {
    let git = find_command("git").unwrap_or_else(|| PathBuf::from("git"));
    let mut args = vec![
        "clone",
        "--depth",
        "1",
        "--recurse-submodules",
        "--shallow-submodules",
    ];
    if let Some(branch) = branch {
        args.extend(["--branch", branch]);
    }
    run_command(
        git.as_os_str(),
        args.into_iter()
            .map(String::from)
            .chain([repo.to_string(), destination.display().to_string()])
            .collect::<Vec<_>>(),
        None,
    )
}

fn build_fishstrap(project_dir: &Path, work_dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let project = project_dir.join("Bloxstrap").join("Bloxstrap.csproj");
    if !project.exists() {
        return Err(format!("couldn't find the Fishstrap project at {}", project.display()).into());
    }
    let artifact_dir = work_dir.join("publish");
    fs::create_dir_all(&artifact_dir)?;
    let dotnet = find_command("dotnet").ok_or("couldn't find the .NET SDK executable")?;
    run_command(
        dotnet.as_os_str(),
        ["restore", "Fishstrap.sln"],
        Some(project_dir),
    )?;
    run_command(
        dotnet.as_os_str(),
        [
            OsString::from("publish"),
            project.as_os_str().to_os_string(),
            OsString::from("-c"),
            OsString::from("Release"),
            OsString::from("-r"),
            OsString::from("win-x64"),
            OsString::from("--self-contained"),
            OsString::from("false"),
            OsString::from("-p:PublishSingleFile=false"),
            OsString::from("-o"),
            artifact_dir.as_os_str().to_os_string(),
        ],
        Some(project_dir),
    )?;
    if !artifact_dir.join(APP_EXE).exists() {
        return Err(format!("publish finished but {} wasn't created", APP_EXE).into());
    }
    Ok(artifact_dir)
}

fn write_injected_files(project_dir: &Path) -> Result<(), Box<dyn Error>> {
    for (relative_path, contents) in INJECTED_FILES {
        let destination = project_dir.join(relative_path.replace('/', "\\"));
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&destination, contents)?;
    }
    Ok(())
}

fn inject_source_changes(project_dir: &Path) -> Result<(), Box<dyn Error>> {
    make_class_partial(
        &project_dir.join("Bloxstrap/Models/Persistable/Settings.cs"),
        "public class Settings",
        "public partial class Settings",
    )?;
    inject_app_startup(project_dir)?;
    inject_settings_navigation(project_dir)?;
    inject_channel_interface(project_dir)?;
    inject_channel_view_model(project_dir)?;
    inject_bootstrapper(project_dir)?;
    Ok(())
}

fn make_class_partial(
    path: &Path,
    original: &str,
    replacement: &str,
) -> Result<(), Box<dyn Error>> {
    let mut contents = read_source(path)?;
    replace_once(&mut contents, original, replacement, path)?;
    fs::write(path, contents)?;
    Ok(())
}

fn inject_app_startup(project_dir: &Path) -> Result<(), Box<dyn Error>> {
    let path = project_dir.join("Bloxstrap/App.xaml.cs");
    let mut contents = read_source(&path)?;
    let marker = "                Settings.Load();\n";
    let addition = r#"                Settings.Load();
                Settings.Prop.EnableAnalytics = false;
                Current.Exit += (_, _) =>
                {
                    if (Settings.Prop.BanAsyncPersistent)
                        return;
                    foreach (string guid in Settings.Prop.BanAsyncSpoofedAdapterGuids.ToList())
                        Utility.BanAsync.MacSpoofer.DeleteNetworkAddressByGuid(guid);
                };
"#;
    replace_once(&mut contents, marker, addition, &path)?;
    fs::write(path, contents)?;
    Ok(())
}

fn inject_settings_navigation(project_dir: &Path) -> Result<(), Box<dyn Error>> {
    let path = project_dir.join("Bloxstrap/UI/Elements/Settings/MainWindow.xaml");
    let mut contents = read_source(&path)?;
    let marker = "                    <ui:NavigationItem Content=\"{x:Static resources:Strings.Common_Deployment}\" PageType=\"{x:Type pages:ChannelPage}\" Icon=\"CloudArchive20\" Tag=\"channel\" />\n";
    let addition = format!(
        "{marker}                    <ui:NavigationItem Content=\"BanAsync\" PageType=\"{{x:Type pages:BanAsyncPage}}\" Icon=\"ShieldKeyhole24\" Tag=\"banasync\" />\n"
    );
    replace_once(&mut contents, marker, &addition, &path)?;
    fs::write(path, contents)?;
    Ok(())
}

fn inject_channel_interface(project_dir: &Path) -> Result<(), Box<dyn Error>> {
    let path = project_dir.join("Bloxstrap/UI/Elements/Settings/Pages/ChannelPage.xaml");
    let mut contents = read_source(&path)?;
    let card_marker = "        <ui:CardExpander Margin=\"0,8,0,0\" IsExpanded=\"True\">\n";
    let toggle = r#"        <controls:OptionControl
            Header="Executor channels"
            Description="Use the Roblox version supported by your selected executor.">
            <ui:ToggleSwitch IsChecked="{Binding UseExecutorChannels, Mode=TwoWay}" />
        </controls:OptionControl>
"#;
    insert_before_once(&mut contents, card_marker, toggle, &path)?;
    let channel_box = "                    <ui:TextBox Grid.Column=\"1\" Margin=\"8,0,8,0\" Padding=\"10,5,10,5\" Width=\"200\" Text=\"{Binding ViewChannel, Mode=TwoWay, UpdateSourceTrigger=PropertyChanged, Delay=250}\" />";
    let selector = r#"                    <StackPanel Grid.Column="1" Margin="8,0,8,0" Width="220">
                        <ui:TextBox Padding="10,5,10,5" Text="{Binding ViewChannel, Mode=TwoWay, UpdateSourceTrigger=PropertyChanged, Delay=250}">
                            <ui:TextBox.Style>
                                <Style TargetType="ui:TextBox" BasedOn="{StaticResource {x:Type ui:TextBox}}">
                                    <Setter Property="IsEnabled" Value="True" />
                                    <Style.Triggers>
                                        <DataTrigger Binding="{Binding UseExecutorChannels}" Value="True">
                                            <Setter Property="IsEnabled" Value="False" />
                                        </DataTrigger>
                                    </Style.Triggers>
                                </Style>
                            </ui:TextBox.Style>
                        </ui:TextBox>
                        <ComboBox Margin="0,6,0,0" Padding="10,5,10,5" MaxDropDownHeight="320" ScrollViewer.CanContentScroll="True" ItemsSource="{Binding Executors}" DisplayMemberPath="Title" SelectedItem="{Binding SelectedExecutor, Mode=TwoWay}">
                            <ComboBox.Style>
                                <Style TargetType="ComboBox" BasedOn="{StaticResource {x:Type ComboBox}}">
                                    <Setter Property="Visibility" Value="Collapsed" />
                                    <Style.Triggers>
                                        <DataTrigger Binding="{Binding UseExecutorChannels}" Value="True">
                                            <Setter Property="Visibility" Value="Visible" />
                                        </DataTrigger>
                                    </Style.Triggers>
                                </Style>
                            </ComboBox.Style>
                        </ComboBox>
                    </StackPanel>"#;
    replace_once(&mut contents, channel_box, selector, &path)?;
    fs::write(path, contents)?;
    Ok(())
}

fn inject_channel_view_model(project_dir: &Path) -> Result<(), Box<dyn Error>> {
    let path = project_dir.join("Bloxstrap/UI/ViewModels/Settings/ChannelViewModel.cs");
    let mut contents = read_source(&path)?;
    replace_once(
        &mut contents,
        "using Bloxstrap.RobloxInterfaces;",
        "using Bloxstrap.Models.APIs;\nusing Bloxstrap.RobloxInterfaces;\nusing System.Collections.ObjectModel;",
        &path,
    )?;
    let constructor = r#"        public ChannelViewModel()
        {
            Task.Run(() => LoadChannelDeployInfo(App.Settings.Prop.Channel));
        }
"#;
    let new_constructor = r#"        public ChannelViewModel()
        {
            Task.Run(InitializeAsync);
        }
        public ObservableCollection<WeaoExploit> Executors { get; } = new();
        private WeaoExploit? _selectedExecutor;
        public WeaoExploit? SelectedExecutor
        {
            get => _selectedExecutor;
            set
            {
                _selectedExecutor = value;
                OnPropertyChanged(nameof(SelectedExecutor));
                if (value is null)
                    return;
                App.Settings.Prop.ExecutorChannel = value.Title;
                Task.Run(() => LoadExecutorDeployInfo(value));
            }
        }
        private async Task InitializeAsync()
        {
            await RefreshExecutorsAsync();
            if (UseExecutorChannels && SelectedExecutor is not null)
                await LoadExecutorDeployInfo(SelectedExecutor);
            else
                await LoadChannelDeployInfo(App.Settings.Prop.Channel);
        }
        private async Task RefreshExecutorsAsync()
        {
            var result = await WeaoClient.GetWindowsExploitsAsync();
            App.Current.Dispatcher.Invoke(() =>
            {
                Executors.Clear();
                foreach (var executor in result.Exploits)
                    Executors.Add(executor);
                SelectedExecutor = Executors.FirstOrDefault(executor =>
                    string.Equals(executor.Title, App.Settings.Prop.ExecutorChannel, StringComparison.OrdinalIgnoreCase));
                if (SelectedExecutor is null && UseExecutorChannels && Executors.Count > 0)
                    SelectedExecutor = Executors[0];
            });
            if (!result.Success && UseExecutorChannels)
            {
                ShowLoadingError = true;
                ChannelInfoLoadingText = result.Error ?? "couldn't load the executor list";
                OnPropertyChanged(nameof(ShowLoadingError));
                OnPropertyChanged(nameof(ChannelInfoLoadingText));
            }
        }
        private Task LoadExecutorDeployInfo(WeaoExploit executor)
        {
            ShowLoadingError = false;
            ShowChannelWarning = !executor.UpdateStatus;
            ChannelInfoLoadingText = $"using {executor.Title}";
            ChannelDeployInfo = new DeployInfo
            {
                Version = string.IsNullOrEmpty(executor.Version) ? "Executor" : executor.Version,
                VersionGuid = executor.RbxVersion,
                Timestamp = string.IsNullOrEmpty(executor.UpdatedDate) ? "?" : executor.UpdatedDate
            };
            OnPropertyChanged(nameof(ShowLoadingError));
            OnPropertyChanged(nameof(ShowChannelWarning));
            OnPropertyChanged(nameof(ChannelInfoLoadingText));
            OnPropertyChanged(nameof(ChannelDeployInfo));
            return Task.CompletedTask;
        }
"#;
    replace_once(&mut contents, constructor, new_constructor, &path)?;
    let update_property = "        public bool UpdateCheckingEnabled\n";
    let executor_property = r#"        public bool UseExecutorChannels
        {
            get => App.Settings.Prop.UseExecutors;
            set
            {
                App.Settings.Prop.UseExecutors = value;
                OnPropertyChanged(nameof(UseExecutorChannels));
                if (value)
                {
                    if (SelectedExecutor is not null)
                        Task.Run(() => LoadExecutorDeployInfo(SelectedExecutor));
                    else
                        Task.Run(RefreshExecutorsAsync);
                }
                else
                {
                    Task.Run(() => LoadChannelDeployInfo(App.Settings.Prop.Channel));
                }
            }
        }
"#;
    insert_before_once(&mut contents, update_property, executor_property, &path)?;
    fs::write(path, contents)?;
    Ok(())
}

fn inject_bootstrapper(project_dir: &Path) -> Result<(), Box<dyn Error>> {
    let path = project_dir.join("Bloxstrap/Bootstrapper.cs");
    let mut contents = read_source(&path)?;
    let version_marker =
        "                _latestVersion = Utilities.ParseVersionSafe(clientVersion.Version);\n";
    let executor_resolution = r#"                _latestVersion = Utilities.ParseVersionSafe(clientVersion.Version);
                if (App.Settings.Prop.UseExecutors && !IsStudioLaunch)
                {
                    string executorName = App.Settings.Prop.ExecutorChannel;
                    var result = await WeaoClient.GetWindowsExploitsAsync();
                    var executor = result.Exploits.FirstOrDefault(item =>
                        string.Equals(item.Title, executorName, StringComparison.OrdinalIgnoreCase));
                    if (executor is null)
                    {
                        Frontend.ShowMessageBox(
                            result.Error ?? $"'{executorName}' isn't available right now. pick another executor in Deployment.",
                            MessageBoxImage.Error,
                            MessageBoxButton.OK);
                        App.Terminate(ErrorCode.ERROR_CANCELLED);
                        return;
                    }
                    _latestVersionGuid = executor.RbxVersion;
                    _latestVersion = null;
                    App.Logger.WriteLine(LOG_IDENT, $"using Roblox version {executor.RbxVersion} for executor '{executor.Title}'");
                }
"#;
    replace_once(&mut contents, version_marker, executor_resolution, &path)?;
    contents = contents.replace(
        "if (App.IsProductionBuild && versionComparison == VersionComparison.Equal || versionComparison == VersionComparison.GreaterThan)",
        "if (versionComparison == VersionComparison.Equal || versionComparison == VersionComparison.GreaterThan)",
    );
    replace_updater_body(&mut contents, &path)?;
    fs::write(path, contents)?;
    Ok(())
}

fn replace_updater_body(contents: &mut String, path: &Path) -> Result<(), Box<dyn Error>> {
    let method_marker = "        private async Task<bool> CheckForUpdates()";
    let method_start = contents
        .find(method_marker)
        .ok_or_else(|| format!("couldn't find the update method in {}", path.display()))?;
    let try_marker = "            try\n            {\n";
    let relative_try = contents[method_start..]
        .find(try_marker)
        .ok_or_else(|| format!("couldn't find the update block in {}", path.display()))?;
    let body_start = method_start + relative_try + try_marker.len();
    let catch_marker = "            catch (Exception ex)";
    let relative_catch = contents[body_start..]
        .find(catch_marker)
        .ok_or_else(|| format!("couldn't find the update error block in {}", path.display()))?;
    let catch_start = body_start + relative_catch;
    let close_marker = "            }\n";
    let close_start = contents[body_start..catch_start]
        .rfind(close_marker)
        .map(|position| body_start + position)
        .ok_or_else(|| format!("couldn't find the end of the update block in {}", path.display()))?;
    let body = r#"                string installerLocation = Path.Combine(Paths.Base, "FishstrapInstaller.exe");
                if (!File.Exists(installerLocation))
                    throw new FileNotFoundException("FishstrapInstaller.exe is missing from the Fishstrap folder", installerLocation);
                ProcessStartInfo startInfo = new()
                {
                    FileName = installerLocation,
                    WorkingDirectory = Paths.Base,
                };
                startInfo.ArgumentList.Add("--repo");
                startInfo.ArgumentList.Add($"https://github.com/{App.ProjectRepository}.git");
                startInfo.ArgumentList.Add("--out");
                startInfo.ArgumentList.Add(Paths.Base);
                startInfo.ArgumentList.Add("--wait-pid");
                startInfo.ArgumentList.Add(Environment.ProcessId.ToString());
                startInfo.ArgumentList.Add("--no-pause");
                foreach (string arg in App.LaunchSettings.Args)
                {
                    startInfo.ArgumentList.Add("--app-arg");
                    startInfo.ArgumentList.Add(arg);
                }
                if (_launchMode == LaunchMode.Player && !App.LaunchSettings.Args.Contains("-player"))
                {
                    startInfo.ArgumentList.Add("--app-arg");
                    startInfo.ArgumentList.Add("-player");
                }
                else if (_launchMode == LaunchMode.Studio && !App.LaunchSettings.Args.Contains("-studio"))
                {
                    startInfo.ArgumentList.Add("--app-arg");
                    startInfo.ArgumentList.Add("-studio");
                }
                App.Settings.Save();
                new InterProcessLock("AutoUpdater");
                Process.Start(startInfo);
                App.SoftTerminate();
                return true;
"#;
    contents.replace_range(body_start..close_start, body);
    Ok(())
}

fn read_source(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(path)?.replace("\r\n", "\n"))
}

fn replace_once(
    contents: &mut String,
    marker: &str,
    replacement: &str,
    path: &Path,
) -> Result<(), Box<dyn Error>> {
    let count = contents.matches(marker).count();
    if count != 1 {
        return Err(format!(
            "expected one source marker in {}, found {count}",
            path.display()
        )
        .into());
    }
    *contents = contents.replacen(marker, replacement, 1);
    Ok(())
}

fn insert_before_once(
    contents: &mut String,
    marker: &str,
    addition: &str,
    path: &Path,
) -> Result<(), Box<dyn Error>> {
    let replacement = format!("{addition}{marker}");
    replace_once(contents, marker, &replacement, path)
}

fn ensure_settings_compatibility(project_dir: &Path) -> Result<(), Box<dyn Error>> {
    let settings_path = project_dir
        .join("Bloxstrap")
        .join("Models")
        .join("Persistable")
        .join("Settings.cs");
    let mut contents = fs::read_to_string(&settings_path)?;
    if contents.contains("RichPresenceStatusDisplayType") {
        return Ok(());
    }
    let class_position = contents.find("class Settings").ok_or_else(|| {
        format!(
            "couldn't find the Settings class in {}",
            settings_path.display()
        )
    })?;
    let opening_brace = contents[class_position..]
        .find('{')
        .map(|position| class_position + position)
        .ok_or_else(|| {
            format!(
                "couldn't find the Settings class body in {}",
                settings_path.display()
            )
        })?;
    let property = "
        public DiscordRPCStatusDisplay RichPresenceStatusDisplayType { get; set; } = DiscordRPCStatusDisplay.Name;
";
    contents.insert_str(opening_brace + 1, property);
    fs::write(&settings_path, contents)?;
    println!(
        "added missing RichPresenceStatusDisplayType compatibility property to {}",
        settings_path.display()
    );
    Ok(())
}

fn copy_dir_contents(source: &Path, destination: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_contents(&source_path, &destination_path)?;
        } else {
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

fn ensure_tool(command: &str, winget_id: &str) -> Result<(), Box<dyn Error>> {
    if find_command(command).is_some() {
        return Ok(());
    }
    println!("{command} isn't installed");
    if find_command("winget").is_none() {
        return Err(format!("install {command} and try again. winget isn't available to install {winget_id} for you").into());
    }
    print!("install {command} with winget ({winget_id})? [Y/n] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    if matches!(answer.trim().to_ascii_lowercase().as_str(), "n" | "no") {
        return Err(format!("can't continue without {command}").into());
    }
    run_command(
        "winget",
        [
            "install",
            "--id",
            winget_id,
            "--exact",
            "--source",
            "winget",
            "--accept-package-agreements",
            "--accept-source-agreements",
        ],
        None,
    )?;
    if find_command(command).is_some() {
        Ok(())
    } else {
        Err(format!(
            "{command} still isn't showing up after install. restart the terminal and try again."
        )
        .into())
    }
}

fn ensure_dotnet_sdk(project_dir: &Path) -> Result<(), Box<dyn Error>> {
    if let Some(dotnet) = find_command("dotnet") {
        let output = Command::new(&dotnet)
            .arg("--version")
            .current_dir(project_dir)
            .output()?;
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout);
            println!("using .NET SDK {}", version.trim());
            return Ok(());
        }
    }
    let requested = read_requested_sdk(project_dir).unwrap_or_else(|| "8.0.100".to_string());
    let major = requested
        .split('.')
        .next()
        .filter(|value| !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()))
        .ok_or_else(|| format!("invalid .NET SDK version in global.json: {requested}"))?;
    let package = format!("microsoft.DotNet.SDK.{major}");
    println!("the required .NET SDK {requested} isn't installed");
    if find_command("winget").is_none() {
        return Err(format!(
            "install .NET SDK {requested} and try again. winget isn't available to install {package} for you."
        )
        .into());
    }
    print!("install .NET SDK {requested} with winget ({package})? [Y/n] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    if matches!(answer.trim().to_ascii_lowercase().as_str(), "n" | "no") {
        return Err(format!("can't continue without .NET SDK {requested}").into());
    }
    run_command(
        "winget",
        [
            "install",
            "--id",
            package.as_str(),
            "--exact",
            "--source",
            "winget",
            "--accept-package-agreements",
            "--accept-source-agreements",
        ],
        None,
    )?;
    let dotnet = find_command("dotnet")
        .ok_or("installed the .NET SDK, but still couldn't find dotnet.exe")?;
    let output = Command::new(&dotnet)
        .arg("--version")
        .current_dir(project_dir)
        .output()?;
    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout);
        println!("using .NET SDK {}", version.trim());
        Ok(())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "the .NET SDK install finished, but this project still can't resolve SDK {requested}: {}",
            error.trim()
        )
        .into())
    }
}

fn read_requested_sdk(project_dir: &Path) -> Option<String> {
    let contents = fs::read_to_string(project_dir.join("global.json")).ok()?;
    let sdk_start = contents.find("\"sdk\"")?;
    let version_offset = contents[sdk_start..].find("\"version\"")?;
    let after_key = &contents[sdk_start + version_offset + "\"version\"".len()..];
    let after_colon = after_key.split_once(':')?.1.trim_start();
    let value = after_colon.strip_prefix('"')?;
    let end = value.find('"')?;
    Some(value[..end].to_string())
}

fn create_start_menu_shortcut(install_dir: &Path) -> Result<(), Box<dyn Error>> {
    let Some(app_data) = env::var_os("APPDATA") else {
        return Ok(());
    };
    let shortcut_dir = PathBuf::from(app_data)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs");
    fs::create_dir_all(&shortcut_dir)?;
    let shortcut_path = shortcut_dir.join("Fishstrap.lnk");
    let target_path = install_dir.join(APP_EXE);
    let script = format!(
        "$shell = New-Object -ComObject WScript.Shell; \
         $shortcut = $shell.CreateShortcut('{}'); \
         $shortcut.TargetPath = '{}'; \
         $shortcut.WorkingDirectory = '{}'; \
         $shortcut.Save()",
        escape_powershell(&shortcut_path),
        escape_powershell(&target_path),
        escape_powershell(install_dir),
    );
    run_command(
        "powershell",
        [
            OsString::from("-NoProfile"),
            OsString::from("-ExecutionPolicy"),
            OsString::from("Bypass"),
            OsString::from("-Command"),
            OsString::from(script),
        ],
        None,
    )
}

fn copy_updater_executable(install_dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let source = env::current_exe()?;
    let destination = install_dir.join(UPDATER_EXE);
    fs::create_dir_all(install_dir)?;
    if source != destination {
        fs::copy(source, &destination)?;
    }
    Ok(destination)
}

fn launch_installed_app(install_dir: &Path, launch_args: &[String]) -> Result<(), Box<dyn Error>> {
    let target_path = install_dir.join(APP_EXE);
    println!("launching {}", target_path.display());
    let mut command = Command::new(&target_path);
    command.current_dir(install_dir);
    command.args(launch_args);
    command.spawn()?;
    Ok(())
}

fn close_running_apps() -> Result<(), Box<dyn Error>> {
    println!("closing running Roblox and Fishstrap stuff");
    let script = format!(
        "$currentPid = {}; \
         $names = @('Fishstrap', 'Fishstrap-QA', 'Bloxstrap', 'Bloxstrap-QA'); \
         $processes = Get-Process -ErrorAction SilentlyContinue | Where-Object {{ \
             $_.Id -ne $currentPid -and ($_.ProcessName -like 'Roblox*' -or $names -contains $_.ProcessName) \
         }}; \
         if ($processes) {{ \
             $ids = @($processes | Select-Object -ExpandProperty Id); \
             $processes | Stop-Process -Force -ErrorAction SilentlyContinue; \
             foreach ($id in $ids) {{ Wait-Process -Id $id -Timeout 10 -ErrorAction SilentlyContinue }} \
         }}",
        std::process::id()
    );
    run_command(
        "powershell",
        [
            OsString::from("-NoProfile"),
            OsString::from("-ExecutionPolicy"),
            OsString::from("Bypass"),
            OsString::from("-Command"),
            OsString::from(script),
        ],
        None,
    )
}

fn register_install(install_dir: &Path) -> Result<(), Box<dyn Error>> {
    let app_path = install_dir.join(APP_EXE);
    let version = powershell_output(&format!(
        "(Get-Item -LiteralPath '{}').VersionInfo.ProductVersion",
        escape_powershell(&app_path)
    ))
    .unwrap_or_default();
    let version = version.trim();
    let install_date = powershell_output("Get-Date -Format yyyyMMdd").unwrap_or_default();
    let script = format!(
        "$key = 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Fishstrap'; \
         New-Item -Path $key -Force | Out-Null; \
         New-ItemProperty -Path $key -Name DisplayIcon -Value '{},0' -PropertyType String -Force | Out-Null; \
         New-ItemProperty -Path $key -Name DisplayName -Value 'Fishstrap' -PropertyType String -Force | Out-Null; \
         New-ItemProperty -Path $key -Name DisplayVersion -Value '{}' -PropertyType String -Force | Out-Null; \
         New-ItemProperty -Path $key -Name InstallDate -Value '{}' -PropertyType String -Force | Out-Null; \
         New-ItemProperty -Path $key -Name InstallLocation -Value '{}' -PropertyType String -Force | Out-Null; \
         New-ItemProperty -Path $key -Name NoRepair -Value 1 -PropertyType DWord -Force | Out-Null; \
         New-ItemProperty -Path $key -Name Publisher -Value 'returnrqt' -PropertyType String -Force | Out-Null; \
         New-ItemProperty -Path $key -Name ModifyPath -Value '\"{}\" -settings' -PropertyType String -Force | Out-Null; \
         New-ItemProperty -Path $key -Name QuietUninstallString -Value '\"{}\" -uninstall -quiet' -PropertyType String -Force | Out-Null; \
         New-ItemProperty -Path $key -Name UninstallString -Value '\"{}\" -uninstall' -PropertyType String -Force | Out-Null; \
         New-ItemProperty -Path $key -Name HelpLink -Value 'https://github.com/bloxstraplabs/bloxstrap/wiki' -PropertyType String -Force | Out-Null; \
         New-ItemProperty -Path $key -Name URLInfoAbout -Value 'https://github.com/returnrqt/fishstrap/issues/new' -PropertyType String -Force | Out-Null; \
         New-ItemProperty -Path $key -Name URLUpdateInfo -Value 'https://github.com/returnrqt/fishstrap/releases' -PropertyType String -Force | Out-Null",
        escape_powershell(&app_path),
        escape_powershell_text(version),
        escape_powershell_text(install_date.trim()),
        escape_powershell(install_dir),
        escape_powershell(&app_path),
        escape_powershell(&app_path),
        escape_powershell(&app_path),
    );
    run_command(
        "powershell",
        [
            OsString::from("-NoProfile"),
            OsString::from("-ExecutionPolicy"),
            OsString::from("Bypass"),
            OsString::from("-Command"),
            OsString::from(script),
        ],
        None,
    )
}

fn powershell_output(script: &str) -> Result<String, Box<dyn Error>> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err("powershell command failed".into())
    }
}

fn wait_for_process_exit(pid: u32) -> Result<(), Box<dyn Error>> {
    println!("waiting for Fishstrap process {pid} to exit before updating files");
    let script = format!(
        "$process = Get-Process -Id {pid} -ErrorAction SilentlyContinue; if ($process) {{ Wait-Process -Id {pid} }}"
    );
    run_command(
        "powershell",
        [
            OsString::from("-NoProfile"),
            OsString::from("-ExecutionPolicy"),
            OsString::from("Bypass"),
            OsString::from("-Command"),
            OsString::from(script),
        ],
        None,
    )
}

fn escape_powershell(path: &Path) -> String {
    path.display().to_string().replace('\'', "''")
}

fn escape_powershell_text(value: &str) -> String {
    value.replace('\'', "''")
}

fn find_command(command: &str) -> Option<PathBuf> {
    if Command::new("cmd")
        .args(["/C", "where", command])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
    {
        return Some(PathBuf::from(command));
    }
    if command.eq_ignore_ascii_case("git") {
        for base in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            let Some(root) = env::var_os(base) else {
                continue;
            };
            let candidate = PathBuf::from(root).join("Git").join("cmd").join("git.exe");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    if command.eq_ignore_ascii_case("dotnet") {
        for base in ["ProgramFiles", "ProgramFiles(x86)"] {
            let Some(root) = env::var_os(base) else {
                continue;
            };
            let candidate = PathBuf::from(root).join("dotnet").join("dotnet.exe");
            if candidate.exists() {
                return Some(candidate);
            }
        }
        if let Some(root) = env::var_os("LOCALAPPDATA") {
            let candidate = PathBuf::from(root)
                .join("Microsoft")
                .join("dotnet")
                .join("dotnet.exe");
            if candidate.exists() {
                return Some(candidate);
            }
        }
        if let Some(root) = env::var_os("USERPROFILE") {
            let candidate = PathBuf::from(root).join(".dotnet").join("dotnet.exe");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn run_command<P, I, S>(
    program: P,
    args: I,
    current_dir: Option<&Path>,
) -> Result<(), Box<dyn Error>>
where
    P: AsRef<OsStr>,
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(program);
    command.args(args);
    if let Some(current_dir) = current_dir {
        command.current_dir(current_dir);
    }
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command failed with status {status}").into())
    }
}

fn timestamp() -> Result<u64, Box<dyn Error>> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn wait_for_enter() {
    eprintln!("press enter to exit.");
    let mut buffer = String::new();
    let _ = io::stdin().read_line(&mut buffer);
}