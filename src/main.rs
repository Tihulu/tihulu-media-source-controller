use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::{Alignment, Length, Limits, Subscription, window::Id};
use cosmic::prelude::*;
use cosmic::widget;
use eframe::{egui, NativeOptions};
use notify_rust::Notification;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, process::Command};

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
    /// Open the full desktop GUI window.
    Gui,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    active_source: Option<String>,
    remember_last_source: bool,
    show_notifications: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            active_source: None,
            remember_last_source: true,
            show_notifications: true,
        }
    }
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

    fn is_playing(&self) -> bool {
        self.status.eq_ignore_ascii_case("Playing")
    }
}

#[derive(Default)]
struct AppletModel {
    core: cosmic::Core,
    popup: Option<Id>,
    config: Config,
    sources: Vec<SourceInfo>,
    last_action: Option<String>,
}

#[derive(Debug, Clone)]
enum AppletMessage {
    TogglePopup,
    PopupClosed(Id),
    Refresh,
    SelectSource(String),
    Previous,
    PlayPause,
    Next,
    Stop,
}

struct DesktopGui {
    config: Config,
    sources: Vec<SourceInfo>,
    status: String,
}

fn main() -> cosmic::iced::Result {
    let first = std::env::args().nth(1);

    match first.as_deref() {
        None | Some("gui") => {
            if let Err(error) = run_desktop_gui() {
                eprintln!("{error}");
                std::process::exit(1);
            }
            Ok(())
        }
        Some("list" | "active" | "set" | "play-pause" | "next" | "previous" | "stop" | "cycle" | "config-path") => {
            if let Err(error) = run_cli() {
                eprintln!("{error}");
                std::process::exit(1);
            }
            Ok(())
        }
        _ => cosmic::applet::run::<AppletModel>(()),
    }
}

fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    let mut config = load_config()?;

    match cli.command {
        Commands::Gui => run_desktop_gui()?,
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

fn run_desktop_gui() -> Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_inner_size([980.0, 660.0])
            .with_min_inner_size([820.0, 560.0]),
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| {
            let mut visuals = egui::Visuals::dark();
            visuals.override_text_color = Some(egui::Color32::from_rgb(240, 240, 245));
            cc.egui_ctx.set_visuals(visuals);
            Ok(Box::new(DesktopGui::new()))
        }),
    )
    .map_err(|error| anyhow!(error.to_string()))
}

impl DesktopGui {
    fn new() -> Self {
        let mut app = Self {
            config: load_config().unwrap_or_default(),
            sources: Vec::new(),
            status: "Ready".to_string(),
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        self.sources = list_source_info();
        self.status = if self.sources.is_empty() {
            "No media sources detected. Open Spotify, VLC, Firefox, or another MPRIS player.".to_string()
        } else if let Some(active) = &self.config.active_source {
            format!("Active source: {active}")
        } else {
            "Select an active media source.".to_string()
        };
    }

    fn set_active(&mut self, source: &str) {
        match set_active_source(&mut self.config, source) {
            Ok(()) => {
                self.status = format!("Media keys now control {source}.");
                self.refresh();
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn command(&mut self, command: &str) {
        match send_to_active(&self.config, command) {
            Ok(()) => {
                self.status = format!("Sent {command} to active source.");
                self.refresh();
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn active_info(&self) -> Option<&SourceInfo> {
        let active = self.config.active_source.as_deref()?;
        self.sources.iter().find(|source| source.name == active)
    }
}

impl eframe::App for DesktopGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(APP_NAME);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Refresh").clicked() {
                        self.refresh();
                    }
                });
            });
            ui.label("Choose one source. Previous, Play/Pause, Next, and Stop will target that source.");
        });

        egui::SidePanel::left("left_panel")
            .resizable(false)
            .exact_width(270.0)
            .show(ctx, |ui| {
                ui.heading("Active Source");
                ui.add_space(6.0);
                if let Some(info) = self.active_info() {
                    ui.label(egui::RichText::new(&info.name).strong().size(18.0));
                    ui.label(info.subtitle());
                } else {
                    ui.label(egui::RichText::new("None selected").weak());
                }

                ui.add_space(16.0);
                ui.heading("Panel Controls");
                ui.horizontal(|ui| {
                    if ui.button("⏮").clicked() {
                        self.command("previous");
                    }
                    if ui.button("⏯").clicked() {
                        self.command("play-pause");
                    }
                    if ui.button("⏭").clicked() {
                        self.command("next");
                    }
                });
                if ui.button("Stop").clicked() {
                    self.command("stop");
                }

                ui.add_space(16.0);
                ui.heading("Status");
                ui.label(&self.status);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Available Sources");
            ui.label("These are MPRIS media sources reported by playerctl.");
            ui.add_space(8.0);

            if self.sources.is_empty() {
                ui.group(|ui| {
                    ui.label("No source found.");
                    ui.label("Open Spotify, VLC, Firefox/YouTube, or another player, then click Refresh.");
                });
                return;
            }

            let sources = self.sources.clone();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for source in sources {
                    let is_active = self.config.active_source.as_deref() == Some(source.name.as_str());
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                let name = if is_active { format!("✓ {}", source.name) } else { source.name.clone() };
                                ui.label(egui::RichText::new(name).strong().size(16.0));
                                ui.label(source.subtitle());
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if is_active {
                                    ui.label("Active");
                                } else if ui.button("Select").clicked() {
                                    self.set_active(&source.name);
                                }
                            });
                        });
                    });
                    ui.add_space(6.0);
                }
            });
        });
    }
}

impl cosmic::Application for AppletModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = AppletMessage;

    const APP_ID: &'static str = "com.github.tihulu.TihuluMediaSourceController";

    fn core(&self) -> &cosmic::Core { &self.core }
    fn core_mut(&mut self) -> &mut cosmic::Core { &mut self.core }

    fn init(core: cosmic::Core, _flags: Self::Flags) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let mut app = Self { core, config: load_config().unwrap_or_default(), ..Default::default() };
        app.refresh_sources();
        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<AppletMessage> { Some(AppletMessage::PopupClosed(id)) }

    fn view(&self) -> Element<'_, Self::Message> {
        self.core
            .applet
            .icon_button("multimedia-player-symbolic")
            .on_press(AppletMessage::TogglePopup)
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> { self.popup_content() }
    fn subscription(&self) -> Subscription<Self::Message> { Subscription::none() }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            AppletMessage::TogglePopup => return self.toggle_popup(),
            AppletMessage::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) { self.popup = None; }
            }
            AppletMessage::Refresh => self.refresh_sources(),
            AppletMessage::SelectSource(source) => match set_active_source(&mut self.config, &source) {
                Ok(()) => { self.last_action = Some(format!("Media keys now control {source}.")); self.refresh_sources(); }
                Err(error) => self.last_action = Some(error.to_string()),
            },
            AppletMessage::Previous => self.media_command("previous"),
            AppletMessage::PlayPause => self.media_command("play-pause"),
            AppletMessage::Next => self.media_command("next"),
            AppletMessage::Stop => self.media_command("stop"),
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> { Some(cosmic::applet::style()) }
}

impl AppletModel {
    fn refresh_sources(&mut self) { self.sources = list_source_info(); }

    fn media_command(&mut self, command: &str) {
        match send_to_active(&self.config, command) {
            Ok(()) => { self.last_action = Some(format!("Sent {command}.")); self.refresh_sources(); }
            Err(error) => self.last_action = Some(error.to_string()),
        }
    }

    fn toggle_popup(&mut self) -> Task<cosmic::Action<AppletMessage>> {
        if let Some(id) = self.popup.take() { return destroy_popup(id); }
        self.refresh_sources();
        let id = Id::unique();
        self.popup = Some(id);
        let mut settings = self.core.applet.get_popup_settings(self.core.main_window_id().unwrap(), id, None, None, None);
        settings.positioner.size_limits = Limits::NONE
            .min_width(430.0)
            .max_width(540.0)
            .min_height(360.0)
            .max_height(760.0);
        get_popup(settings)
    }

    fn popup_content(&self) -> Element<'_, AppletMessage> {
        let active = self.config.active_source.clone().unwrap_or_else(|| "None selected".to_string());
        let mut content = widget::column::with_capacity(16)
            .spacing(12)
            .padding(14)
            .push(widget::text::title3(APP_NAME))
            .push(widget::text("Choose which media source the media controls target."))
            .push(widget::divider::horizontal::light())
            .push(widget::text::title4(format!("Active Source: {active}")));

        if let Some(action) = &self.last_action {
            content = content.push(widget::container(widget::text(action.clone())).padding(8));
        }

        let mut list = widget::column::with_capacity(self.sources.len().max(1)).spacing(8);
        if self.sources.is_empty() {
            list = list.push(widget::container(widget::text("No MPRIS players found. Start Spotify, VLC, Firefox, or another media app, then refresh.")).padding(10));
        } else {
            for source in &self.sources {
                list = list.push(applet_source_row(source, self.config.active_source.as_deref() == Some(source.name.as_str())));
            }
        }

        content = content
            .push(widget::scrollable(list).height(Length::Fixed(360.0)).width(Length::Fill))
            .push(widget::divider::horizontal::light())
            .push(widget::row::with_children(vec![
                widget::button::text("Refresh").on_press(AppletMessage::Refresh).into(),
                widget::button::text("Previous").on_press(AppletMessage::Previous).into(),
                widget::button::text("Play / Pause").on_press(AppletMessage::PlayPause).into(),
                widget::button::text("Next").on_press(AppletMessage::Next).into(),
                widget::button::text("Stop").on_press(AppletMessage::Stop).into(),
            ]).spacing(8).align_y(Alignment::Center));

        self.core.applet.popup_container(content).into()
    }
}

fn applet_source_row(source: &SourceInfo, active: bool) -> Element<'_, AppletMessage> {
    let action: Element<'_, AppletMessage> = if active {
        widget::text("Active").into()
    } else {
        widget::button::text("Select").on_press(AppletMessage::SelectSource(source.name.clone())).into()
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

fn config_path() -> Result<PathBuf> {
    let base = dirs::config_dir().ok_or_else(|| anyhow!("Could not find user config directory"))?;
    Ok(base.join("tihulu-media-source-controller").join("config.toml"))
}

fn load_config() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() { return Ok(Config::default()); }
    let raw = fs::read_to_string(path).context("Failed to read config")?;
    toml::from_str::<Config>(&raw).context("Failed to parse config")
}

fn save_config(config: &Config) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).context("Failed to create config directory")?; }
    let raw = toml::to_string_pretty(config).context("Failed to serialize config")?;
    fs::write(path, raw).context("Failed to write config")?;
    Ok(())
}

fn list_sources() -> Result<Vec<String>> {
    let output = Command::new("playerctl").arg("--list-all").output().context("playerctl is required. Install it with: sudo apt install playerctl")?;
    if !output.status.success() { return Ok(Vec::new()); }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut sources: Vec<String> = stdout.lines().map(str::trim).filter(|line| !line.is_empty()).map(String::from).collect();
    sources.sort();
    sources.dedup();
    Ok(sources)
}

fn list_source_info() -> Vec<SourceInfo> {
    list_sources().unwrap_or_default().into_iter().map(|name| {
        let status = playerctl_text(&name, &["status"]).unwrap_or_else(|| "Unknown".to_string());
        let title = playerctl_text(&name, &["metadata", "title"]).unwrap_or_default();
        let artist = playerctl_text(&name, &["metadata", "artist"]).unwrap_or_default();
        SourceInfo { name, status, title, artist }
    }).collect()
}

fn playerctl_text(source: &str, args: &[&str]) -> Option<String> {
    let mut command = Command::new("playerctl");
    command.arg("--player").arg(source);
    for arg in args { command.arg(arg); }
    let output = command.output().ok()?;
    if !output.status.success() { return None; }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

fn resolve_source(input: &str, sources: &[String]) -> Result<String> {
    if sources.iter().any(|source| source == input) { return Ok(input.to_string()); }
    let input_lower = input.to_lowercase();
    let matches: Vec<&String> = sources.iter().filter(|source| source.to_lowercase().contains(&input_lower)).collect();
    match matches.as_slice() {
        [single] => Ok((*single).clone()),
        [] => Err(anyhow!("No source matching '{input}' was found")),
        many => Err(anyhow!("Source name is ambiguous. Matches: {}", many.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))),
    }
}

fn set_active_source(config: &mut Config, source: &str) -> Result<()> {
    config.active_source = Some(source.to_string());
    if !config.remember_last_source { config.remember_last_source = true; }
    save_config(config)?;
    notify(config, "Media source changed", &format!("Media keys now control {source}."));
    Ok(())
}

fn send_to_active(config: &Config, command: &str) -> Result<()> {
    let source = config.active_source.as_deref().ok_or_else(|| anyhow!("No active source selected. Choose one source first."))?;
    let status = Command::new("playerctl").arg("--player").arg(source).arg(command).status().with_context(|| format!("Failed to execute playerctl for source '{source}'"))?;
    if status.success() { Ok(()) } else { Err(anyhow!("playerctl command failed for source '{source}'")) }
}

fn next_source(active: Option<&str>, sources: &[String]) -> String {
    if sources.is_empty() { return String::new(); }
    let current_index = active.and_then(|name| sources.iter().position(|source| source == name));
    let next_index = match current_index { Some(index) => (index + 1) % sources.len(), None => 0 };
    sources[next_index].clone()
}

fn notify(config: &Config, summary: &str, body: &str) {
    if config.show_notifications {
        let _ = Notification::new().summary(summary).body(body).appname(APP_NAME).show();
    }
}
