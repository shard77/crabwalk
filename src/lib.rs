mod app;
mod config;
mod github;
mod keys;
mod markdown;
mod types;
mod ui;

use anyhow::Result;
use crossterm::event::EventStream;
use futures_util::StreamExt;
use ratatui::DefaultTerminal;
use std::{sync::Arc, time::Duration};
use tokio::time::{Instant, MissedTickBehavior};

const TICK_PERIOD: Duration = Duration::from_millis(200);
// Limit each batch so incoming results cannot starve input and redraws.
const APP_EVENT_BATCH_SIZE: usize = 32;

/// Starts the terminal app.
pub fn run() -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;

    // Octocrab starts background tasks while building the client, so enter the runtime first.
    let _runtime_context = runtime.enter();
    let config = config::Config::load();
    let github = config
        .token
        .as_deref()
        .map(github::GitHubClient::new)
        .transpose()?
        .map(Arc::new);

    let mut app = app::App::new(github, config.prefs);
    if let Some(error) = config.load_error {
        app.modal = app::Modal::Error(format!(
            "Could not load config; using defaults.\n\n{error:#}"
        ));
    }

    ratatui::run(|terminal| {
        runtime.block_on(async {
            app.load_initial_prs();
            run_loop(terminal, &mut app).await
        })
    })
}

async fn run_loop(terminal: &mut DefaultTerminal, app: &mut app::App) -> Result<()> {
    let mut input = EventStream::new();
    let mut ticks = tokio::time::interval_at(Instant::now() + TICK_PERIOD, TICK_PERIOD);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut needs_redraw = true;
    let mut batch = Vec::with_capacity(APP_EVENT_BATCH_SIZE);

    while !app.navigation.should_quit {
        while batch.len() < APP_EVENT_BATCH_SIZE {
            let Ok(event) = app.background.events.try_recv() else {
                break;
            };
            batch.push(event);
        }
        if !batch.is_empty() {
            app.handle_app_events(batch.drain(..));
            needs_redraw = true;
        }
        if needs_redraw {
            terminal.draw(|frame| ui::draw(frame, app))?;
            needs_redraw = false;
        }

        tokio::select! {
            event = input.next() => {
                match event {
                    Some(event) => needs_redraw = keys::handle_event(app, event?),
                    None => break,
                }
            }
            Some(event) = app.background.events.recv() => batch.push(event),
            _ = ticks.tick() => needs_redraw = app.tick(),
        }
    }
    Ok(())
}
