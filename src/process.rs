use std::path::PathBuf;
use std::time::Duration;
use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::process::Command;

const IDLE_CPU_THRESHOLD: f32 = 30.0;
const IDLE_POLL_INTERVAL_SECS: u64 = 5;
const POST_STOP_DELAY_SECS: u64 = 5;
const IDLE_TIMEOUT_SECS: u64 = 300;

pub struct Process {
    exec_path: PathBuf,
    child: Option<tokio::process::Child>,
    pid: Option<u32>,
}

impl Process {
    pub fn new(exec_path: PathBuf) -> Self {
        Self {
            exec_path,
            child: None,
            pid: None,
        }
    }

    pub async fn start(&mut self) -> std::io::Result<()> {
        if self.child.is_some() || self.pid.is_some() {
            self.stop().await;
        }

        let child = Command::new(&self.exec_path)
            .args(["-steam", "-game", "garrysmod", "-w", "0", "-h", "0"])
            .spawn()?;

        self.pid = child.id();
        self.child = Some(child);
        println!("Process started.");
        Ok(())
    }

    pub async fn stop(&mut self) {
        // Try tree kill using the saved PID first
        if let Some(pid) = self.pid.take() {
            self.kill_process_tree(pid).await;
        }

        // Also drop the child handle
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
        }

        // Fall back: kill any remaining processes by executable name
        self.kill_by_name();

        println!("Process stopped.");
        tokio::time::sleep(Duration::from_secs(POST_STOP_DELAY_SECS)).await;
    }

    async fn kill_process_tree(&self, pid: u32) {
        #[cfg(target_os = "windows")]
        {
            let _ = tokio::process::Command::new("taskkill")
                .args(["/T", "/F", "/PID", &pid.to_string()])
                .output()
                .await;
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Kill the specific process, then its children via kill_by_name fallback
            let _ = tokio::process::Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .output()
                .await;
        }
    }

    fn kill_by_name(&self) {
        let proc_name = match self.exec_path.file_name() {
            Some(name) => name.to_owned(),
            None => return,
        };

        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);

        for process in system.processes_by_name(&proc_name) {
            process.kill();
        }
    }

    /// Finds child processes spawned by the launcher PID.
    /// The launcher (gmod.exe) spawns the real game process and exits.
    /// We look for processes whose parent is our launcher PID, or if the
    /// launcher already exited, any process matching the executable name.
    fn find_game_pids(&self, system: &System) -> Vec<Pid> {
        let proc_name = match self.exec_path.file_name() {
            Some(name) => name,
            None => return vec![],
        };

        system
            .processes_by_name(proc_name)
            .map(|p| p.pid())
            .collect()
    }

    pub async fn wait_for_idle(&mut self) {
        let mut system = System::new();
        let start_time = std::time::Instant::now();
        let timeout = Duration::from_secs(IDLE_TIMEOUT_SECS);

        println!("Waiting for idle...");

        // Give the launcher a moment to spawn the real game process
        tokio::time::sleep(Duration::from_secs(IDLE_POLL_INTERVAL_SECS)).await;

        loop {
            if start_time.elapsed() >= timeout {
                eprintln!("Timed out waiting for process to idle.");
                break;
            }

            system.refresh_processes(ProcessesToUpdate::All, true);

            let game_pids = self.find_game_pids(&system);

            // If no matching processes exist, the game has exited
            if game_pids.is_empty() {
                break;
            }

            // Sum CPU usage across all matching game processes
            let total_cpu: f32 = game_pids
                .iter()
                .filter_map(|pid| system.process(*pid))
                .map(|p| p.cpu_usage())
                .sum();

            if total_cpu <= IDLE_CPU_THRESHOLD {
                break;
            }

            tokio::time::sleep(Duration::from_secs(IDLE_POLL_INTERVAL_SECS)).await;
        }

        println!("Process is idling.");
    }
}
