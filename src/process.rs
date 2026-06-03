use std::ffi::OsStr;
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::process::Command;

use crate::config::Settings;

const IDLE_POLL_INTERVAL_SECS: u64 = 2;
const POST_STOP_DELAY_SECS: u64 = 5;
const STARTUP_TIMEOUT_SECS: u64 = 120;
/// Safety cap on the Linux read-based idle wait.
#[cfg(target_os = "linux")]
const IDLE_TIMEOUT_SECS: u64 = 300;
/// Consecutive low-read samples required before the game is considered done
/// loading (debounces a brief lull during loading). Linux only.
#[cfg(target_os = "linux")]
const IDLE_LOW_SAMPLES: u32 = 2;
/// Ignore reads for this long after the process appears, so a quiet moment
/// before asset loading begins isn't mistaken for "finished loading". Linux only.
#[cfg(target_os = "linux")]
const IDLE_STARTUP_GRACE_SECS: u64 = 5;
/// Platforms without `/proc` can't measure disk reads, so they wait a fixed
/// period long enough to reach the menu before closing the game.
#[cfg(not(target_os = "linux"))]
const NONLINUX_SETTLE_SECS: u64 = 60;

pub struct Process {
    process_names: Vec<String>,
    steam_app_id: u32,
    /// Only consulted on Linux (the read-based idle path).
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    idle_read_threshold_mb_s: f64,
}

impl Process {
    pub fn new(settings: &Settings) -> Self {
        Self {
            process_names: settings.process_names.clone(),
            steam_app_id: settings.steam_app_id,
            idle_read_threshold_mb_s: settings.idle_read_threshold_mb_s,
        }
    }

    pub async fn start(&self) -> std::io::Result<()> {
        // Launch through Steam's URL handler. Steam resolves the install
        // location and applies the user's launch options, and every platform
        // needs only the app id — no path to the executable. The opener process
        // hands off to the running Steam client and exits immediately; it is
        // never the game itself, so we don't hold on to it.
        let url = format!("steam://rungameid/{}", self.steam_app_id);
        let status = Self::url_opener(&url).status().await?;
        if !status.success() {
            eprintln!("URL opener exited with {status}; Steam may still launch the game.");
        }

        println!("Process started.");
        Ok(())
    }

    /// Builds the platform-appropriate command for opening a `steam://` URL.
    fn url_opener(url: &str) -> Command {
        #[cfg(target_os = "windows")]
        let cmd = {
            // The empty argument is `start`'s window-title placeholder; without
            // it the quoted URL would be swallowed as the title.
            let mut c = Command::new("cmd");
            c.args(["/C", "start", "", url]);
            c
        };
        #[cfg(target_os = "macos")]
        let cmd = {
            let mut c = Command::new("open");
            c.arg(url);
            c
        };
        #[cfg(target_os = "linux")]
        let cmd = {
            let mut c = Command::new("xdg-open");
            c.arg(url);
            c
        };
        cmd
    }

    pub async fn stop(&self) {
        // We launch through the steam:// handler and never hold a handle to the
        // game, so the only way to stop it is by its executable name. Killing
        // the opener would be pointless — it has already exited.
        self.kill_by_name();

        println!("Process stopped.");
        tokio::time::sleep(Duration::from_secs(POST_STOP_DELAY_SECS)).await;
    }

    fn kill_by_name(&self) {
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);

        for name in &self.process_names {
            for process in system.processes_by_name(OsStr::new(name)) {
                process.kill();
            }
        }
    }

    /// Finds running processes matching any of the configured executable names.
    /// Note: this matches by name only, so a child process spawned by the
    /// launcher under a different name will not be tracked.
    fn find_game_pids(&self, system: &System) -> Vec<Pid> {
        let mut pids = Vec::new();
        for name in &self.process_names {
            for process in system.processes_by_name(OsStr::new(name)) {
                // Names can overlap (substring matches), so guard against dups.
                let pid = process.pid();
                if !pids.contains(&pid) {
                    pids.push(pid);
                }
            }
        }
        pids
    }

    /// Waits for the game to launch and then settle into idle (or exit).
    /// Returns `false` if the game process never appeared within the startup
    /// timeout, so the caller can treat the attempt as a failure rather than a
    /// completed open/close cycle.
    pub async fn wait_for_idle(&self) -> bool {
        let mut system = System::new();

        // Phase 1: wait for the game process to appear. Launching through Steam
        // and Proton can take far longer than a direct exec, so we can't assume
        // the process exists shortly after start().
        println!("Waiting for game to launch...");
        let startup_deadline = Instant::now() + Duration::from_secs(STARTUP_TIMEOUT_SECS);
        loop {
            system.refresh_processes(ProcessesToUpdate::All, true);
            if !self.find_game_pids(&system).is_empty() {
                break;
            }
            if Instant::now() >= startup_deadline {
                eprintln!("Game process never appeared within {STARTUP_TIMEOUT_SECS}s.");
                return false;
            }
            tokio::time::sleep(Duration::from_secs(IDLE_POLL_INTERVAL_SECS)).await;
        }

        // Phase 2: wait for the game to finish loading, or exit.
        //
        // CPU is a poor signal: a focused 3D menu keeps it busy rendering, so it
        // never looks idle unless minimized. On Linux we watch disk reads
        // instead (see `wait_until_loaded`); other platforms fall back to a fixed
        // settle.
        println!("Waiting for game to finish loading...");
        self.wait_until_loaded(&mut system).await;

        println!("Process is idling.");
        true
    }

    /// Linux: treat the game as loaded once disk-read throughput stays low.
    ///
    /// Loading reads hundreds of MB of assets; the instant the menu appears that
    /// drops to ~zero, regardless of window focus. We track `rchar` (bytes read
    /// via read syscalls, so it counts even on a warm page cache) for the game's
    /// thread-group leaders, resolved once here so we poll `/proc` directly
    /// instead of re-enumerating every process each iteration.
    #[cfg(target_os = "linux")]
    async fn wait_until_loaded(&self, system: &mut System) {
        let mut leader_set = std::collections::HashSet::new();
        for pid in self.find_game_pids(system) {
            if let Some(tgid) = Self::read_tgid(pid) {
                leader_set.insert(tgid);
            }
        }
        let leaders: Vec<u32> = leader_set.into_iter().collect();

        let threshold_bytes_per_s = self.idle_read_threshold_mb_s * 1_000_000.0;
        let safety_deadline = Instant::now() + Duration::from_secs(IDLE_TIMEOUT_SECS);
        let grace_deadline = Instant::now() + Duration::from_secs(IDLE_STARTUP_GRACE_SECS);
        let mut prev_read = Self::total_read_chars(&leaders).unwrap_or(0);
        let mut prev_at = Instant::now();
        let mut low_streak = 0u32;

        loop {
            tokio::time::sleep(Duration::from_secs(IDLE_POLL_INTERVAL_SECS)).await;

            // No leader left in /proc means the game exited on its own.
            let cur_read = match Self::total_read_chars(&leaders) {
                Some(read) => read,
                None => break,
            };

            let now = Instant::now();
            let elapsed = now.duration_since(prev_at).as_secs_f64();
            let rate = if elapsed > 0.0 {
                cur_read.saturating_sub(prev_read) as f64 / elapsed
            } else {
                f64::INFINITY
            };
            prev_read = cur_read;
            prev_at = now;

            // Don't evaluate during the startup grace window.
            if now < grace_deadline {
                continue;
            }

            if rate < threshold_bytes_per_s {
                low_streak += 1;
                if low_streak >= IDLE_LOW_SAMPLES {
                    break;
                }
            } else {
                low_streak = 0;
            }

            if now >= safety_deadline {
                eprintln!("Timed out waiting for game to finish loading.");
                break;
            }
        }
    }

    /// Non-Linux: no `/proc` to measure disk reads, so wait a fixed period long
    /// enough to reach the menu, breaking early if the game exits.
    #[cfg(not(target_os = "linux"))]
    async fn wait_until_loaded(&self, system: &mut System) {
        let settle_deadline = Instant::now() + Duration::from_secs(NONLINUX_SETTLE_SECS);
        loop {
            tokio::time::sleep(Duration::from_secs(IDLE_POLL_INTERVAL_SECS)).await;

            system.refresh_processes(ProcessesToUpdate::All, true);
            if self.find_game_pids(system).is_empty() {
                break; // game exited on its own
            }
            if Instant::now() >= settle_deadline {
                break;
            }
        }
    }

    /// Sums `rchar` across the given thread-group leaders, returning `None` when
    /// none of them still exist (the game has exited). `rchar` is used rather
    /// than physical `read_bytes`, which reads ~zero from a warm page cache and
    /// would make loading look idle.
    #[cfg(target_os = "linux")]
    fn total_read_chars(leaders: &[u32]) -> Option<u64> {
        let mut sum = 0u64;
        let mut any_alive = false;
        for &tgid in leaders {
            if let Some(rchar) = Self::read_rchar(tgid) {
                any_alive = true;
                sum += rchar;
            }
        }
        any_alive.then_some(sum)
    }

    /// Reads the thread-group id for `pid` from `/proc/<pid>/status`, collapsing
    /// the engine's worker threads onto their owning process so their I/O isn't
    /// counted more than once.
    #[cfg(target_os = "linux")]
    fn read_tgid(pid: Pid) -> Option<u32> {
        Self::proc_field(format!("/proc/{pid}/status"), "Tgid:")?
            .parse()
            .ok()
    }

    /// Reads cumulative `rchar` for a thread-group leader from `/proc/<tgid>/io`.
    #[cfg(target_os = "linux")]
    fn read_rchar(tgid: u32) -> Option<u64> {
        Self::proc_field(format!("/proc/{tgid}/io"), "rchar:")?
            .parse()
            .ok()
    }

    /// Reads the trimmed value of the first `key` line in a `/proc` file.
    #[cfg(target_os = "linux")]
    fn proc_field(path: String, key: &str) -> Option<String> {
        let contents = std::fs::read_to_string(path).ok()?;
        contents
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .map(|value| value.trim().to_owned())
    }
}
