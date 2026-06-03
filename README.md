<div align="center">
  <img src="./assets/sm.jpg"/>
  <h1>Startup Millenium</h1>
</div>

A simple program to get the acheivement for a game called Garry's Mod. This acheivement requires you to open and close the game 1000 times.  

You can find the latest release [Here](https://github.com/gage-lodba/StartupMillenium/releases/latest).

## Configuration
A `config.json` is created next to the executable after the first run:

```json
{
  "steam_app_id": 4000,
  "process_names": ["hl2_linux", "gmod"],
  "idle_read_threshold_mb_s": 10.0
}
```

The game is launched through Steam's `steam://rungameid/<steam_app_id>` URL handler, so Steam resolves the install location itself — **no game path is required, on any platform.** The app id is the same on every branch, so launching works whatever beta you've selected.

- `steam_app_id` — the Steam app id of the game (`4000` for Garry's Mod).
- `process_names` — the running process(es) used to detect when the game is up and to close it. All listed names are matched, so a single config can cover more than one branch. Accepts either a single string (`"gmod.exe"`) or a list. The name depends on the branch and how Steam runs it:

  | Branch / runtime | Process name |
  | --- | --- |
  | Windows (any branch) | `gmod.exe` |
  | Linux — default (32-bit) | `hl2_linux` |
  | Linux — **x86-64** branch | `gmod` |
  | Linux — forced Proton | `gmod.exe` |

  The Linux default lists both `hl2_linux` and `gmod`, so the same config works on the default and x86-64 branches. If your setup differs, set `process_names` to whatever `pgrep -af gmod` (or `cat /proc/<pid>/comm`) reports while the game is running.
- `idle_read_threshold_mb_s` — read throughput (MB/s) below which the game is treated as done loading and closed. The tool watches disk read activity rather than CPU: loading reads assets at hundreds of MB/s, and once the menu appears that drops to ~zero — even while the window is focused (CPU, by contrast, stays high at the menu and only falls when minimized). Default `10.0`; raise it if your machine has heavy background read activity, lower it (above zero) to be stricter. Must be positive — a value of `0` or less is rejected and reset to the default. *(Linux only — without `/proc`, other platforms instead wait a fixed ~60s settle before closing.)*

> [!NOTE]
> `hl2_linux` is the shared Source-engine binary name, so matching it could also close another native Source game if one is running. Narrow `process_names` (e.g. to just `["gmod"]` on the x86-64 branch) if that matters to you.

> [!NOTE]
> Steam must be installed and its `steam://` URL handler registered (the default after any normal Steam install). Any per-game launch flags should be set in the game's *Launch Options* in the Steam client.
