mod config;
mod process;
#[cfg(test)]
mod tests;

use config::Settings;
use process::Process;

#[tokio::main]
async fn main() {
    let settings = match Settings::initialize().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to load config: {e}");
            return;
        }
    };

    if !settings.game_path.exists() {
        eprintln!(
            "Game executable not found: {}",
            settings.game_path.display()
        );
        eprintln!("Please update the path in config.json and restart.");
        return;
    }

    let mut process = Process::new(settings.game_path);

    let mut successes = 0;
    let mut failures = 0;
    let mut total_attempts = 0;
    while successes < 1000 {
        total_attempts += 1;
        if let Err(e) = process.start().await {
            failures += 1;
            eprintln!("Failed to start process (attempt {total_attempts}): {e}");

            if failures >= 3 {
                eprintln!("Too many consecutive failures, aborting.");
                return;
            }

            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            continue;
        }
        failures = 0;
        process.wait_for_idle().await;
        process.stop().await;
        successes += 1;
        println!("{}/1000", successes);
    }

    println!("Finished");
}
