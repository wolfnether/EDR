use std::collections::HashMap;

use crossterm::event::KeyCode;
use rayon::prelude::{IntoParallelRefMutIterator, ParallelIterator};
use tokio::io::{stdin, AsyncReadExt};

use crate::data::{Server, Station, SteamPlayer, SteamPlayers, StopDescription, Train};

pub struct State<'a> {
    pub station_translation: HashMap<&'a str, String>,
    pub station_name_to_prefix: HashMap<&'a str, String>,

    pub servers: Vec<Server>,
    pub server_index: usize,
    pub selected_server: String,

    pub stations: Vec<Station>,
    pub station_index: usize,

    pub selected_station: Option<Station>,
    pub players: Vec<SteamPlayer>,

    pub step: Step,
    pub events: Vec<Event>,
}

#[allow(clippy::upper_case_acronyms)]
pub enum Step {
    ServerSelection,
    StationSelection,
    EDR,
}

#[derive(Eq, PartialEq)]
pub struct Event {
    pub name: String,
    pub time: isize,
    pub planned_time: isize,
    pub ty: EventType,

    pub player: bool,

    pub prev: String,
    pub next: String,
}

impl Event {
    pub fn get_time(&self) -> String {
        if self.planned_time == self.time {
            format!("{:0>2}:{:0>2}", self.time / 60, self.time % 60)
        } else {
            format!(
                "{:0>2}:{:0>2} ({:+})",
                self.time / 60,
                self.time % 60,
                self.time - self.planned_time
            )
        }
    }
}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        todo!()
    }
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.planned_time != 0 {
            self.time.partial_cmp(&other.time)
        } else if other.planned_time == 0 {
            Some(core::cmp::Ordering::Equal)
        } else {
            Some(core::cmp::Ordering::Greater)
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum EventType {
    Passing,
    Entering,
    Departing,
}

impl<'a> State<'a> {
    pub async fn new() -> crate::Result<State<'a>> {
        let servers = reqwest::get("https://staging.simrail.deadlykungfu.ninja/servers")
            .await?
            .json()
            .await?;

        Ok(Self {
            station_translation: ron::from_str(include_str!("../stations.ron")).unwrap(),
            station_name_to_prefix: ron::from_str(include_str!("../station_prefix.ron")).unwrap(),
            servers,
            server_index: 0,
            selected_server: String::new(),

            stations: vec![],
            station_index: 0,
            selected_station: None,

            players: vec![],

            step: Step::ServerSelection,
            events: vec![],
        })
    }

    pub async fn refresh_data(&mut self) -> crate::Result<()> {
        match self.step {
            Step::ServerSelection => {
                self.servers = reqwest::get("https://staging.simrail.deadlykungfu.ninja/servers")
                    .await?
                    .json()
                    .await?
            }

            Step::StationSelection => {
                self.stations = reqwest::get(format!(
                    "https://staging.simrail.deadlykungfu.ninja/stations/{}",
                    self.selected_server
                ))
                .await?
                .json()
                .await?;

                let player = self
                    .stations
                    .iter()
                    .flat_map(|s| &s.dispatched_by)
                    .map(|by| by.steam_id.clone())
                    .collect::<Vec<_>>();

                if !player.is_empty() {
                    self.players = reqwest::get(format!(
                        "https://panel.simrail.eu:8084/users-open/{}",
                        player.join(",")
                    ))
                    .await?
                    .json::<SteamPlayers>()
                    .await?
                    .players;
                }
            }
            Step::EDR => {
                self.events.clear();
                let mut trains: Vec<Train> = reqwest::get(format!(
                    "https://staging.simrail.deadlykungfu.ninja/trains/{}",
                    self.selected_server
                ))
                .await?
                .json()
                .await?;

                for train in trains.iter_mut() {
                    if let Some((nearest_station, _)) = self
                        .stations
                        .iter()
                        .map(|s| (s, train.dist_from(s)))
                        .reduce(|(sa, d1), (sb, d2)| match d1.total_cmp(&d2) {
                            core::cmp::Ordering::Less => (sa, d1),
                            core::cmp::Ordering::Equal => (sa, d1),
                            core::cmp::Ordering::Greater => (sb, d2),
                        })
                    {
                        let loc = nearest_station.prefix.clone();
                        train.loc = Some(loc.clone());

                        let timetable: Vec<StopDescription> = reqwest::get(format!(
                            "https://staging.simrail.deadlykungfu.ninja/train/{}",
                            train.train_no,
                        ))
                        .await?
                        .json()
                        .await?;

                        if let Some(train_pos) = timetable.iter().position(|s| {
                            if let Some(prefix) = self.get_prefix(&s.station) {
                                prefix == &loc
                            } else {
                                false
                            }
                        }) {
                            if let Some(station_pos) = timetable.iter().position(|s| {
                                if let Some(prefix) = self.get_prefix(&s.station) {
                                    prefix
                                        == &self
                                            .selected_station
                                            .as_ref()
                                            .expect("no selected station")
                                            .prefix
                                } else {
                                    false
                                }
                            }) {
                                if train_pos <= station_pos {
                                    let stop = &timetable[station_pos];
                                    let next_stop = &timetable[station_pos + 1];
                                    let prev_stop = &timetable[station_pos - 1];
                                    if let Some(stop_type) = stop.stop_type.as_ref() {
                                        if stop_type == "ph" {
                                            let arrival = stop
                                                .scheduled_arrival_hour
                                                .as_ref()
                                                .map_or(0, |time| {
                                                    let time = time.split(':').collect::<Vec<_>>();
                                                    time[0].parse::<isize>().unwrap() * 60
                                                        + time[1].parse::<isize>().unwrap()
                                                });

                                            let departure = stop
                                                .scheduled_departure_hour
                                                .as_ref()
                                                .map_or(0, |time| {
                                                    let time = time.split(':').collect::<Vec<_>>();
                                                    time[0].parse::<isize>().unwrap() * 60
                                                        + time[1].parse::<isize>().unwrap()
                                                });
                                            self.events.push(Event {
                                                name: format!(
                                                    "{} {}",
                                                    train.train_name, train.train_no
                                                ),
                                                time: arrival,
                                                planned_time: arrival,
                                                ty: EventType::Entering,
                                                prev: format!(
                                                    "{}/L.{}",
                                                    prev_stop.station,
                                                    prev_stop.line.clone(),
                                                ),
                                                next: format!("platform ({})", departure - arrival),
                                                player: train.t != "bot",
                                            });
                                            self.events.push(Event {
                                                name: format!(
                                                    "{} {}",
                                                    train.train_name, train.train_no
                                                ),
                                                time: departure,
                                                planned_time: departure,
                                                ty: EventType::Departing,
                                                prev: "Platform".to_string(),
                                                next: format!(
                                                    "{}/L.{}",
                                                    next_stop.station,
                                                    stop.line.clone(),
                                                ),
                                                player: train.t != "bot",
                                            });
                                        }
                                    } else {
                                        let passing = stop.scheduled_arrival_hour.as_ref().map_or(
                                            0,
                                            |time| {
                                                let time = time.split(':').collect::<Vec<_>>();
                                                time[0].parse::<isize>().unwrap() * 60
                                                    + time[1].parse::<isize>().unwrap()
                                            },
                                        );

                                        self.events.push(Event {
                                            name: format!(
                                                "{} {}",
                                                train.train_name, train.train_no
                                            ),
                                            time: passing,
                                            planned_time: passing,
                                            ty: EventType::Passing,
                                            prev: format!(
                                                "{}/L.{}",
                                                prev_stop.station,
                                                prev_stop.line.clone(),
                                            ),
                                            next: format!(
                                                "{}/L.{}",
                                                next_stop.station,
                                                stop.line.clone(),
                                            ),
                                            player: train.t != "bot",
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_player_name(&self, steam_id: Option<&String>) -> Option<&String> {
        if let Some(steam_id) = steam_id {
            self.players
                .iter()
                .find(|p| &p.steam_id == steam_id)
                .map(|p| &p.steam_info[0].personaname)
        } else {
            None
        }
    }

    pub fn key_pressed(&mut self, key_code: KeyCode) -> (bool, bool) {
        match key_code {
            KeyCode::Enter => self.select(),
            KeyCode::Up => self.cursor(-1),
            KeyCode::Down => self.cursor(1),
            KeyCode::Esc => match self.step {
                Step::ServerSelection => (false, false),
                Step::StationSelection => {
                    self.step = Step::ServerSelection;
                    (true, false)
                }
                Step::EDR => {
                    self.step = Step::StationSelection;
                    (true, false)
                }
            },
            _ => (false, false),
        }
    }

    fn select(&mut self) -> (bool, bool) {
        match self.step {
            Step::ServerSelection => {
                self.selected_server = self.servers[self.server_index].server_code.clone();
                self.step = Step::StationSelection;
                (true, true)
            }
            Step::StationSelection => {
                self.selected_station = Some(self.stations[self.station_index].clone());
                self.step = Step::EDR;
                (true, true)
            }
            Step::EDR => (false, false),
        }
    }

    fn cursor(&mut self, i: isize) -> (bool, bool) {
        match self.step {
            Step::ServerSelection => {
                let mut res = (self.server_index as isize) + i;
                if res < 0 {
                    res = (self.servers.len() - 1) as _;
                } else if res >= self.servers.len() as isize {
                    res = 0;
                }
                self.server_index = res as _;
                (false, true)
            }
            Step::StationSelection => {
                let mut res = (self.station_index as isize) + i;
                if res < 0 {
                    res = (self.stations.len() - 1) as _;
                } else if res >= self.stations.len() as isize {
                    res = 0;
                }
                self.station_index = res as _;
                (false, true)
            }
            Step::EDR => (false, false),
        }
    }

    fn get_prefix(&self, station: &str) -> Option<&String> {
        self.station_name_to_prefix.get(station)
    }
}
