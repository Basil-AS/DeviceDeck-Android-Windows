# Install DeviceDeck on Linux

Pair your earbuds in the system's Bluetooth settings first. DeviceDeck connects to paired earbuds.

## Script (recommended)

```sh
curl -fsSL https://raw.githubusercontent.com/Basil-AS/DeviceDeck-Android-Windows/develop/scripts/install-linux.sh | sh
```

It needs only `curl` and BlueZ, and no administrator rights. It puts the AppImage in `~/.local/share/quickbuds` and adds a `quickbuds` command and a menu entry. The app updates itself.

## Remove it

```sh
curl -fsSL https://raw.githubusercontent.com/Basil-AS/DeviceDeck-Android-Windows/develop/scripts/install-linux.sh | sh -s uninstall
```

## Without the script

Download the file from the [latest release](https://github.com/Basil-AS/DeviceDeck-Android-Windows/releases):

- `DeviceDeck-linux-x64.AppImage`: make it executable and run it.
