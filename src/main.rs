#![feature(async_closure)]
use core::fmt::Display;
use core::time::Duration;
use std::process;
use std::time::Instant;

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use state::State;
use tui::backend::{Backend, CrosstermBackend};
use tui::layout::Constraint;
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, List, ListItem, ListState, Row, Table};
use tui::{Frame, Terminal};

pub type Result<T> = core::result::Result<T, Box<dyn std::error::Error>>;

mod data;
mod state;

macro_rules! exit_on_error {
    ($to_test:expr,$terminal:expr) => {
        if let Err(err) = $to_test {
            exit(&mut $terminal, Some(err))?;
        }
    };
}

#[tokio::main]
async fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let refresh_data = Duration::from_secs_f32(5.0);
    let mut last_tick = Instant::now();

    let mut state = State::new().await?;

    let mut need_refresh_data = false;
    let mut need_refresh_tui = false;

    exit_on_error!(terminal.draw(|f| draw(f, &mut state)), terminal);

    loop {
        let timeout = refresh_data
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == crossterm::event::KeyEventKind::Release {
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => exit::<String>(&mut terminal, None)?,
                    _ => {
                        (need_refresh_data, need_refresh_tui) = state.key_pressed(key.code);
                    }
                }
            }
        }

        if need_refresh_data || last_tick.elapsed() >= refresh_data {
            exit_on_error!(state.refresh_data().await, terminal);

            last_tick = Instant::now();
            need_refresh_data = false;
            need_refresh_tui = true;
        }

        if need_refresh_tui {
            exit_on_error!(terminal.draw(|f| draw(f, &mut state)), terminal);
        }
    }
}

fn exit<E: Display>(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    err: Option<E>,
) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    if let Some(err) = err {
        eprintln!("{err}");
    }
    process::exit(0)
}

pub fn draw<B: Backend>(f: &mut Frame<B>, state: &mut State) {
    match state.step {
        state::Step::ServerSelection => draw_server_selection(f, state),
        state::Step::StationSelection => draw_station_selection(f, state),
        state::Step::EDR => draw_edr(f, state),
    }
}

fn draw_edr<B: Backend>(f: &mut Frame<B>, state: &mut State) {
    state.events.sort();
    f.render_widget(
        Table::new(state.events.iter().enumerate().map(|(i, e)| {
            Row::new(vec![
                if e.player { '*' } else { ' ' }.to_string(),
                e.name.clone(),
                match e.ty {
                    state::EventType::Passing => "",
                    state::EventType::Entering => "IN",
                    state::EventType::Departing => "OUT",
                }
                .to_string(),
                e.get_time(),
                e.prev.clone(),
                e.next.clone(),
            ])
            .style(Style::default().add_modifier(Modifier::UNDERLINED))
        }))
        .header(Row::new(vec!["", "Train", "", "Time", "From", "To"]))
        .widths(&[
            Constraint::Length(2),
            Constraint::Percentage(30),
            Constraint::Length(4),
            Constraint::Length(6),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .block(Block::default().borders(Borders::ALL).title(
            format!(" {}/{} ",state.selected_server,state
                    .selected_station
                    .as_ref()
                    .expect("selected station is none")
                    .name
                    .clone(),),
        )),
        f.size(),
    )
}

fn draw_station_selection<B: Backend>(f: &mut Frame<B>, state: &State) {
    let mut _state = ListState::default();
    _state.select(Some(state.station_index));

    f.render_stateful_widget(
        List::new(
            state
                .stations
                .iter()
                .map(|s| {
                    ListItem::new(format!(
                        "{} \t {}{}{}",
                        s.prefix,
                        s.name,
                        if s.dispatched_by.is_empty() {
                            ""
                        } else {
                            " - "
                        },
                        s.dispatched_by
                            .iter()
                            .flat_map(|s| state.get_player_name(Some(&s.steam_id)))
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("/")
                    ))
                    .style(Style::default().add_modifier(
                        if s.dispatched_by.is_empty() {
                            Modifier::empty()
                        } else {
                            Modifier::BOLD
                        },
                    ))
                })
                .collect::<Vec<_>>(),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {}/Station Selection ", state.selected_server)),
        ),
        f.size(),
        &mut _state,
    );
}

fn draw_server_selection<B: Backend>(f: &mut Frame<B>, state: &State) {
    let mut _state = ListState::default();
    _state.select(Some(state.server_index));

    f.render_stateful_widget(
        List::new(
            state
                .servers
                .iter()
                .map(|s| {
                    ListItem::new(format!("{} {}", s.server_code, s.server_name)).style(
                        Style::default().add_modifier({
                            let mut modifier = Modifier::empty();
                            if !s.is_active {
                                modifier.toggle(Modifier::CROSSED_OUT)
                            }
                            modifier
                        }),
                    )
                })
                .collect::<Vec<_>>(),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Server Selection"),
        ),
        f.size(),
        &mut _state,
    );
}
