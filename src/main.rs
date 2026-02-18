mod config;
mod process;
#[cfg(test)]
mod tests;

use config::Settings;
use process::Process;
use tokio::io::AsyncBufReadExt;

#[tokio::main]
async fn main() {
    if let Err(e) = Settings::initialize().await {
        eprintln!("Failed to load config: {e}");
        return;
    }

    let settings = Settings::get_settings().clone();

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
    while successes < 1000 {
        if let Err(e) = process.start().await {
            failures += 1;
            eprintln!(
                "Failed to start process (attempt {}): {e}",
                successes + failures
            );

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

    let mut input = String::new();
    let _ = tokio::io::BufReader::new(tokio::io::stdin())
        .read_line(&mut input)
        .await;
}
