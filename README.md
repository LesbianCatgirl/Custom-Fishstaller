# Fishstrap Installer

A small installer for building and installing a customized version of [Fishstrap](https://github.com/returnrqt/fishstrap.git) built in rust.

## Custom stuff

The installer patches a few specific parts of the Fishstrap source so minor file changes wont break it (i hope)

If Fishstrap changes something that one of the integrations depends on, the installer stops and reports the affected file instead of trying to patch it.

## Updates

The customized Fishstrap build uses `FishstrapInstaller.exe` for updates instead of the normal Fishstrap release

When an update is detected, Fishstrap starts the installer from its install folder. The installer waits for Fishstrap to close, downloads the latest source reapplies the custom integrations and replaces the installed files with the new ones

**The app doesn't auto update you need to re-install from the new github executable to update**

## Building

Build the installer with:

```bat
cargo build --release
```

The executable will be created at:

```text
target\release\fishstrap-installer.exe
```

from the workspace path

## Running

```bat
fishstrap-installer.exe
```

Available arguments:

```text
--repo <url>         Fishstrap repo to clone
--branch <name>      Branch to clone
--out <dir>          Installation directory
--keep-temp          Keep the temporary source folder
--build-only         Build and test integrations without installing
--wait-pid <pid>     Wait for a process to exit before updating files
--no-pause           Dont pause after errors
```

You can also set:

```text
FISHSTRAP_REPO
FISHSTRAP_BRANCH
```

# Credits
 1. Fishtrap
 2. Ban Async tab logic - Exploit Strap
 3. The rest is made by me, (Yes that means the executor Channels was made by me, I made it like a year before exploit strap came out)

## Note to self
For testing changes against a newer Fishstrap version, tis is useful:

```bat
fishstrap-installer.exe --build-only --keep-temp
```