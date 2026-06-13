use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use notify_rust::Notification;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, process::Command};

const APP_NAME: &str = "COSMIC Media Source Controller";

#[derive(Parser, Debug)]
#[command(name = "cosmic-media-source-controller")]
#[command(about = "Route Linux media keys to one selected media source", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List available MPRIS media players reported by playerctl.
    List,
    /// Show the currently selected media source.
    Active,
    /// Select the active media source.
    Set { source: String },
    /// Send Play/Pause to the selected source.
    PlayPause,
    /// Send Next to the selected source.
    Next,
    /// Send Previous to the selected source.
    Previous,
    /// Send Stop to the selected source.
    Stop,
    /// Toggle between available sources.
    Cycle,
    /// Print the config file path.
    ConfigPath,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Config {
    active_source: Option<String>,
    remember_last_source: bool,
    show_notifications: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut config = load_config()?;

    match cli.command {
        Commands::List => {
            for source in list_sources()? {
                let marker = if config.active_source.as_deref() == Some(&source) { "*" } else { " " };
                println!("{marker} {source}");
            }
        }
        Commands::Active => match &config.active_source {
            Some(source) => println!("{source}"),
            None => println!("No active source selected"),
        },
        Commands::Set { source } => {
            let sources = list_sources()?;
            let selected = resolve_source(&source, &sources)?;
            config.active_source = Some(selected.clone());
            if !config.remember_last_source {
                config.remember_last_source = true;
            }
            if !config.show_notifications {
                config.show_notifications = true;
            }
            save_config(&config)?;
            notify(&config, "Media source changed", &format!("Media keys now control {selected}."));
            println!("Active source: {selected}");
        }
        Commands::PlayPause => send_to_active(&config, "play-pause")?,
        Commands::Next => send_to_active(&config, "next")?,
        Commands::Previous => send_to_active(&config, "previous")?,
        Commands::Stop => send_to_active(&config, "stop")?,
        Commands::Cycle => {
            let sources = list_sources()?;
            if sources.is_empty() {
                return Err(anyhow!("No MPRIS media players found"));
            }
            let next = next_source(config.active_source.as_deref(), &sources);
            config.active_source = Some(next.clone());
            save_config(&config)?;
            notify(&config, "Media source changed", &format!("Media keys now control {next}."));
            println!("Active source: {next}");
        }
        Commands::ConfigPath => println!("{}", config_path()?.display()),
    }

    Ok(())
}

fn config_path() -> Result<PathBuf> {
    let base = dirs::config_dir().ok_or_else(|| anyhow!("Could not find user config directory"))?;
    Ok(base.join("cosmic-media-source-controller").join("config.toml"))
}

fn load_config() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(Config {
            active_source: None,
            remember_last_source: true,
            show_notifications: true,
        });
    }
    let raw = fs::read_to_string(path).context("Failed to read config")?;
    let config = toml::from_str::<Config>(&raw).context("Failed to parse config")?;
    Ok(config)
}

fn save_config(config: &Config) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create config directory")?;
    }
    let raw = toml::to_string_pretty(config).context("Failed to serialize config")?;
    fs::write(path, raw).context("Failed to write config")?;
    Ok(())
}

fn list_sources() -> Result<Vec<String>> {
    let output = Command::new("playerctl")
        .arg("--list-all")
        .output()
        .context("playerctl is required. Install it with: sudo apt install playerctl")?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut sources: Vec<String> = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect();
    sources.sort();
    sources.dedup();
    Ok(sources)
}

fn resolve_source(input: &str, sources: &[String]) -> Result<String> {
    if sources.iter().any(|source| source == input) {
        return Ok(input.to_string());
    }

    let input_lower = input.to_lowercase();
    let matches: Vec<&String> = sources
        .iter()
        .filter(|source| source.to_lowercase().contains(&input_lower))
        .collect();

    match matches.as_slice() {
        [single] => Ok((*single).clone()),
        [] => Err(anyhow!("No source matching '{input}' was found")),
        many => Err(anyhow!(
            "Source name is ambiguous. Matches: {}",
            many.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        )),
    }
}

fn send_to_active(config: &Config, command: &str) -> Result<()> {
    let source = config
        .active_source
        .as_deref()
        .ok_or_else(|| anyhow!("No active source selected. Run: cosmic-media-source-controller set <source>"))?;

    let status = Command::new("playerctl")
        .arg("--player")
        .arg(source)
        .arg(command)
        .status()
        .with_context(|| format!("Failed to execute playerctl for source '{source}'"))?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("playerctl command failed for source '{source}'"))
    }
}

fn next_source(active: Option<&str>, sources: &[String]) -> String {
    if sources.is_empty() {
        return String::new();
    }

    let current_index = active.and_then(|name| sources.iter().position(|source| source == name));
    let next_index = match current_index {
        Some(index) => (index + 1) % sources.len(),
        None => 0,
    };
    sources[next_index].clone()
}

fn notify(config: &Config, summary: &str, body: &str) {
    if config.show_notifications {
        let _ = Notification::new()
            .summary(summary)
            .body(body)
            .appname(APP_NAME)
            .show();
    }
}
