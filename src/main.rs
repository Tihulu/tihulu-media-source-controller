use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::{Alignment, Length, Limits, Subscription, window::Id};
use cosmic::prelude::*;
use cosmic::widget;
use notify_rust::Notification;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, process::Command};

const APP_ID: &str = "com.github.tihulu.TihuluMediaSourceController";
const APP_NAME: &str = "Tihulu Media Source Controller";
const APP_COMMAND: &str = "tihulu-media-source-controller";

#[derive(Parser, Debug)]
#[command(name = APP_COMMAND)]
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

#[derive(Debug, Clone, Default)]
struct SourceInfo {
    name: String,
    status: String,
    title: String,
    artist: String,
}

impl SourceInfo {
    fn subtitle(&self) -> String {
        let mut parts = Vec::new();
        if !self.status.is_empty() {
            parts.push(self.status.clone());
        }
        if !self.title.is_empty() {
            parts.push(self.title.clone());
        }
        if !self.artist.is_empty() {
            parts.push(self.artist.clone());
        }

        if parts.is_empty() {
            "No metadata".to_string()
        } else {
            parts.join(" • ")
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum PopupKind {
    Sources,
    NowPlaying,
    Settings,
}

#[derive(Default)]
struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    popup_kind: Option<PopupKind>,
    config: Config,
    sources: Vec<SourceInfo>,
    last_action: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    ToggleSources,
    ToggleNowPlaying,
    ToggleSettings,
    PopupClosed(Id),
    Refresh,
    SelectSource(String),
    Previous,
    PlayPause,
    Next,
    Stop,
    ToggleRemember(bool),
    ToggleNotifications(bool),
}

fn main() -> cosmic::iced::Result {
    if std::env::args_os().len() > 1 {
        if let Err(error) = run_cli() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }

    cosmic::applet::run::<AppModel>(())
}

fn run_cli() -> Result<()> {
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
            set_active_source(&mut config, &selected)?;
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
            set_active_source(&mut config, &next)?;
            println!("Active source: {next}");
        }
        Commands::ConfigPath => println!("{}", config_path()?.display()),
    }

    Ok(())
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(core: cosmic::Core, _flags: Self::Flags) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let mut app = Self {
            core,
            config: load_config().unwrap_or_default(),
            ..Default::default()
        };
        app.refresh_sources();
        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn view(&self) -> Element<'_, Self::Message> {
        widget::row::with_children(vec![
            widget::button::text("⏮").on_press(Message::Previous).into(),
            widget::button::text("⏯").on_press(Message::PlayPause).into(),
            widget::button::text("⏭").on_press(Message::Next).into(),
            self.core
                .applet
                .icon_button("view-list-symbolic")
                .on_press(Message::ToggleSources)
                .into(),
        ])
        .spacing(4)
        .align_y(Alignment::Center)
        .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        match self.popup_kind.unwrap_or(PopupKind::Sources) {
            PopupKind::Sources => self.sources_popup(),
            PopupKind::NowPlaying => self.now_playing_popup(),
            PopupKind::Settings => self.settings_popup(),
        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::none()
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::ToggleSources => return self.toggle_popup(PopupKind::Sources),
            Message::ToggleNowPlaying => return self.toggle_popup(PopupKind::NowPlaying),
            Message::ToggleSettings => return self.toggle_popup(PopupKind::Settings),
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                    self.popup_kind = None;
                }
            }
            Message::Refresh => self.refresh_sources(),
            Message::SelectSource(source) => match set_active_source(&mut self.config, &source) {
                Ok(()) => {
                    self.last_action = Some(format!("Media keys now control {source}."));
                    self.refresh_sources();
                }
                Err(error) => self.last_action = Some(error.to_string()),
            },
            Message::Previous => self.media_command("previous"),
            Message::PlayPause => self.media_command("play-pause"),
            Message::Next => self.media_command("next"),
            Message::Stop => self.media_command("stop"),
            Message::ToggleRemember(value) => {
                self.config.remember_last_source = value;
                self.persist_settings();
            }
            Message::ToggleNotifications(value) => {
                self.config.show_notifications = value;
                self.persist_settings();
            }
        }

        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl AppModel {
    fn refresh_sources(&mut self) {
        self.sources = list_source_info();
    }

    fn persist_settings(&mut self) {
        if let Err(error) = save_config(&self.config) {
            self.last_action = Some(error.to_string());
        }
    }

    fn media_command(&mut self, command: &str) {
        match send_to_active(&self.config, command) {
            Ok(()) => {
                self.last_action = Some(format!("Sent {command} to active source."));
                self.refresh_sources();
            }
            Err(error) => self.last_action = Some(error.to_string()),
        }
    }

    fn active_info(&self) -> Option<&SourceInfo> {
        let active = self.config.active_source.as_deref()?;
        self.sources.iter().find(|source| source.name == active)
    }

    fn toggle_popup(&mut self, kind: PopupKind) -> Task<cosmic::Action<Message>> {
        if self.popup_kind == Some(kind) {
            self.popup_kind = None;
            if let Some(id) = self.popup.take() {
                return destroy_popup(id);
            }
            return Task::none();
        }

        let mut tasks = Vec::new();
        if let Some(id) = self.popup.take() {
            tasks.push(destroy_popup(id));
        }

        self.refresh_sources();
        let id = Id::unique();
        self.popup = Some(id);
        self.popup_kind = Some(kind);

        let mut settings = self.core.applet.get_popup_settings(
            self.core.main_window_id().unwrap(),
            id,
            None,
            None,
            None,
        );

        settings.positioner.size_limits = match kind {
            PopupKind::Sources => Limits::NONE
                .min_width(420.0)
                .max_width(520.0)
                .min_height(360.0)
                .max_height(760.0),
            PopupKind::NowPlaying => Limits::NONE
                .min_width(380.0)
                .max_width(480.0)
                .min_height(260.0)
                .max_height(520.0),
            PopupKind::Settings => Limits::NONE
                .min_width(360.0)
                .max_width(460.0)
                .min_height(220.0)
                .max_height(480.0),
        };

        tasks.push(get_popup(settings));
        Task::batch(tasks)
    }

    fn sources_popup(&self) -> Element<'_, Message> {
        let mut content = widget::column::with_capacity(16)
            .spacing(12)
            .padding(14)
            .push(header_row("Tihulu Media Source Controller", "Choose the active media source."));

        if let Some(active) = &self.config.active_source {
            content = content.push(card_text("Active Source", active));
        } else {
            content = content.push(card_text("Active Source", "None selected"));
        }

        if let Some(action) = &self.last_action {
            content = content.push(widget::container(widget::text(action.clone())).padding(8));
        }

        content = content.push(widget::divider::horizontal::light());
        content = content.push(widget::text::title4("Available Sources"));

        let mut list = widget::column::with_capacity(self.sources.len().max(1)).spacing(8);
        if self.sources.is_empty() {
            list = list.push(widget::container(widget::text(
                "No MPRIS players detected. Start Spotify, VLC, Firefox, or another media app, then refresh.",
            ))
            .padding(10));
        } else {
            for source in &self.sources {
                let active = self.config.active_source.as_deref() == Some(source.name.as_str());
                list = list.push(source_row(source, active));
            }
        }

        content = content
            .push(widget::scrollable(list).height(Length::Fixed(360.0)).width(Length::Fill))
            .push(widget::divider::horizontal::light())
            .push(
                widget::row::with_children(vec![
                    widget::button::text("Refresh").on_press(Message::Refresh).into(),
                    widget::button::text("Now Playing")
                        .on_press(Message::ToggleNowPlaying)
                        .into(),
                    widget::button::text("Settings")
                        .on_press(Message::ToggleSettings)
                        .into(),
                ])
                .spacing(10)
                .align_y(Alignment::Center),
            );

        self.core.applet.popup_container(content).into()
    }

    fn now_playing_popup(&self) -> Element<'_, Message> {
        let mut content = widget::column::with_capacity(10)
            .spacing(14)
            .padding(14)
            .push(header_row("Now Playing", "Controls always target the selected source."));

        match self.active_info() {
            Some(info) => {
                content = content
                    .push(card_text(&info.name, &info.subtitle()))
                    .push(
                        widget::row::with_children(vec![
                            widget::button::text("Previous").on_press(Message::Previous).into(),
                            widget::button::text("Play / Pause")
                                .on_press(Message::PlayPause)
                                .into(),
                            widget::button::text("Next").on_press(Message::Next).into(),
                            widget::button::text("Stop").on_press(Message::Stop).into(),
                        ])
                        .spacing(8)
                        .align_y(Alignment::Center),
                    );
            }
            None => {
                content = content.push(card_text(
                    "No active source",
                    "Open Source Picker and choose a media source first.",
                ));
            }
        }

        if let Some(action) = &self.last_action {
            content = content.push(widget::container(widget::text(action.clone())).padding(8));
        }

        content = content.push(
            widget::row::with_children(vec![
                widget::button::text("Source Picker")
                    .on_press(Message::ToggleSources)
                    .into(),
                widget::button::text("Settings")
                    .on_press(Message::ToggleSettings)
                    .into(),
            ])
            .spacing(10),
        );

        self.core.applet.popup_container(content).into()
    }

    fn settings_popup(&self) -> Element<'_, Message> {
        let content = widget::column::with_capacity(10)
            .spacing(14)
            .padding(14)
            .push(header_row("Settings", "Minimal behavior controls."))
            .push(settings_switch_row(
                "Remember last source",
                self.config.remember_last_source,
                Message::ToggleRemember,
            ))
            .push(settings_switch_row(
                "Show notification on source change",
                self.config.show_notifications,
                Message::ToggleNotifications,
            ))
            .push(widget::divider::horizontal::light())
            .push(card_text(
                "Shortcut commands",
                "play-pause, previous, next, stop, cycle",
            ))
            .push(
                widget::row::with_children(vec![
                    widget::button::text("Source Picker")
                        .on_press(Message::ToggleSources)
                        .into(),
                    widget::button::text("Now Playing")
                        .on_press(Message::ToggleNowPlaying)
                        .into(),
                ])
                .spacing(10),
            );

        self.core.applet.popup_container(content).into()
    }
}

fn header_row<'a>(title: &'a str, subtitle: &'a str) -> Element<'a, Message> {
    widget::column::with_children(vec![
        widget::text::title3(title).into(),
        widget::text(subtitle).into(),
    ])
    .spacing(3)
    .into()
}

fn card_text<'a>(title: &'a str, body: &'a str) -> Element<'a, Message> {
    widget::container(
        widget::column::with_children(vec![
            widget::text::title4(title).into(),
            widget::text(body).into(),
        ])
        .spacing(4),
    )
    .padding(10)
    .width(Length::Fill)
    .into()
}

fn source_row(source: &SourceInfo, active: bool) -> Element<'_, Message> {
    let action: Element<'_, Message> = if active {
        widget::text("Active").into()
    } else {
        widget::button::text("Select")
            .on_press(Message::SelectSource(source.name.clone()))
            .into()
    };

    widget::container(
        widget::row::with_children(vec![
            widget::column::with_children(vec![
                widget::text::title4(source.name.clone()).into(),
                widget::text(source.subtitle()).into(),
            ])
            .spacing(3)
            .width(Length::Fill)
            .into(),
            action,
        ])
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding(10)
    .width(Length::Fill)
    .into()
}

fn settings_switch_row(
    label: &'static str,
    value: bool,
    on_toggle: fn(bool) -> Message,
) -> Element<'static, Message> {
    widget::row::with_children(vec![
        widget::text(label).into(),
        widget::Space::new().width(Length::Fill).into(),
        widget::toggler(value).on_toggle(on_toggle).into(),
    ])
    .align_y(Alignment::Center)
    .into()
}

fn config_path() -> Result<PathBuf> {
    let base = dirs::config_dir().ok_or_else(|| anyhow!("Could not find user config directory"))?;
    Ok(base.join("tihulu-media-source-controller").join("config.toml"))
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
    toml::from_str::<Config>(&raw).context("Failed to parse config")
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

fn list_source_info() -> Vec<SourceInfo> {
    list_sources()
        .unwrap_or_default()
        .into_iter()
        .map(|name| {
            let status = playerctl_text(&name, &["status"]).unwrap_or_else(|| "Unknown".to_string());
            let title = playerctl_text(&name, &["metadata", "title"]).unwrap_or_default();
            let artist = playerctl_text(&name, &["metadata", "artist"]).unwrap_or_default();
            SourceInfo {
                name,
                status,
                title,
                artist,
            }
        })
        .collect()
}

fn playerctl_text(source: &str, args: &[&str]) -> Option<String> {
    let mut command = Command::new("playerctl");
    command.arg("--player").arg(source);
    for arg in args {
        command.arg(arg);
    }

    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
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

fn set_active_source(config: &mut Config, source: &str) -> Result<()> {
    config.active_source = Some(source.to_string());
    if !config.remember_last_source {
        config.remember_last_source = true;
    }
    save_config(config)?;
    notify(config, "Media source changed", &format!("Media keys now control {source}."));
    Ok(())
}

fn send_to_active(config: &Config, command: &str) -> Result<()> {
    let source = config
        .active_source
        .as_deref()
        .ok_or_else(|| anyhow!("No active source selected. Click the applet and choose one source first."))?;

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
