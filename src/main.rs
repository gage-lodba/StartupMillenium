mod config;
mod process;
#[cfg(test)]
mod tests;

use std::time::Duration;

use config::Settings;
use process::Process;

/// Number of completed open/close cycles the achievement requires.
const SUCCESS_TARGET: u32 = 1000;
/// Consecutive failed attempts before giving up.
const MAX_CONSECUTIVE_FAILURES: u32 = 3;
/// Backoff between attempts after a failure.
const RETRY_DELAY_SECS: u64 = 5;

#[tokio::main]
async fn main() {
    let settings = match Settings::initialize().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to load config: {e}");
            return;
        }
    };

    let process = Process::new(&settings);

    let mut successes = 0u32;
    let mut failures = 0u32;
    let mut attempt = 0u32;
    while successes < SUCCESS_TARGET {
        attempt += 1;

        if run_cycle(&process, attempt).await {
            failures = 0;
            successes += 1;
            println!("{successes}/{SUCCESS_TARGET}");
            continue;
        }

        failures += 1;
        if failures >= MAX_CONSECUTIVE_FAILURES {
            eprintln!("Too many consecutive failures, aborting.");
            return;
        }
        tokio::time::sleep(Duration::from_secs(RETRY_DELAY_SECS)).await;
    }

    println!("Finished");
}

/// Runs one open → wait-until-loaded → close cycle. Returns `true` on a
/// completed cycle, `false` if the game failed to start or never appeared.
async fn run_cycle(process: &Process, attempt: u32) -> bool {
    if let Err(e) = process.start().await {
        eprintln!("Failed to start process (attempt {attempt}): {e}");
        return false;
    }

    let launched = process.wait_for_idle().await;
    process.stop().await;

    if !launched {
        eprintln!("Game did not launch (attempt {attempt}).");
        return false;
    }
    true
}
