use chrono::{DateTime, Timelike, Utc};
use crossterm::event::KeyCode;

use crate::data::{
    Server, ServerResponse, Station, StationResponse, SteamPlayer, SteamPlayers, StopDescription,
    Train, TrainResponse,
};

pub struct State {
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

#[derive(Eq, PartialEq, Debug)]
pub struct Event {
    pub name: String,
    pub time: Option<DateTime<Utc>>,
    pub planned_time: DateTime<Utc>,
    pub ty: EventType,

    pub player: bool,

    pub prev: String,
    pub next: String,
}

impl Event {
    pub fn get_time(&self) -> String {
        if let Some(time) = self.time {
            todo!()
        } else {
            format!(
                "{:0>2}:{:0>2}",
                self.planned_time.hour(),
                self.planned_time.minute()
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
        let a_time = self.time.unwrap_or(self.planned_time);
        let b_time = other.time.unwrap_or(other.planned_time);

        a_time.partial_cmp(&b_time)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum EventType {
    Passing,
    Entering,
    Departing,
}

impl State {
    pub async fn new() -> crate::Result<State> {
        let servers = get_servers().await?;

        Ok(Self {
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
                self.servers = get_servers().await?;
            }

            Step::StationSelection => {
                self.stations = reqwest::get(format!(
                    "https://panel.simrail.eu:8084/stations-open?serverCode={}",
                    self.selected_server
                ))
                .await?
                .json::<StationResponse>()
                .await?
                .data;

                self.stations.sort_by(|a, b| a.name.cmp(&b.name));

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
                    "https://panel.simrail.eu:8084/trains-open?serverCode={}",
                    self.selected_server
                ))
                .await?
                .json::<TrainResponse>()
                .await?
                .data;

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
                        let loc = nearest_station.name.clone();
                        train.loc = Some(loc.clone());

                        let mut timetable: Vec<StopDescription> = reqwest::get(format!(
                            "https://simrail-edr.emeraldnetwork.xyz/train/{}/{}",
                            self.selected_server, train.train_no,
                        ))
                        .await?
                        .json()
                        .await?;

                        timetable.sort_by(|a, b| a.indexOfPoint.cmp(&b.indexOfPoint));

                        if let Some(train_pos) = timetable.iter().position(|s| s.nameOfPoint == loc)
                        {
                            if let Some(station_pos) = timetable.iter().position(|s| {
                                s.nameOfPoint
                                    == self
                                        .selected_station
                                        .as_ref()
                                        .expect("no station selected")
                                        .name
                            }) {
                                if train_pos <= station_pos {
                                    let stop = &timetable[station_pos];
                                    let next_stop = if station_pos + 1 != timetable.len() {
                                        &timetable[station_pos + 1]
                                    } else {
                                        //todo something better
                                        &timetable[station_pos]
                                    };
                                    let prev_stop = if station_pos != 0 {
                                        &timetable[station_pos - 1]
                                    } else {
                                        //todo something better
                                        &timetable[station_pos]
                                    };

                                    if stop.plannedStop.unwrap_or_default() == 0 {
                                        self.events.push(Event {
                                            name: format!(
                                                "{} {}",
                                                train.train_name, train.train_no
                                            ),
                                            time: stop
                                                .actualArrivalTime
                                                .as_ref()
                                                .map(|_| stop.actualArrivalObject),
                                            planned_time: stop.scheduledArrivalObject,
                                            ty: EventType::Passing,
                                            player: train.t != "bot",
                                            prev: format!(
                                                "{}/L.{}",
                                                prev_stop.nameOfPoint, prev_stop.line
                                            ),
                                            next: format!(
                                                "{}/L.{}",
                                                next_stop.nameOfPoint, stop.line
                                            ),
                                        })
                                    } else {
                                        self.events.push(Event {
                                            name: format!(
                                                "{} {}",
                                                train.train_name, train.train_no
                                            ),
                                            time: stop
                                                .actualArrivalTime
                                                .as_ref()
                                                .map(|_| stop.actualArrivalObject),
                                            planned_time: stop.scheduledArrivalObject,
                                            ty: EventType::Entering,
                                            player: train.t != "bot",
                                            prev: format!(
                                                "{}/L.{}",
                                                prev_stop.nameOfPoint, prev_stop.line
                                            ),
                                            next: if let (Some(platform), Some(track)) =
                                                (stop.platform.as_ref(), stop.track)
                                            {
                                                format!("{}/{}", platform, track)
                                            } else {
                                                String::from("Not a plaform stop!")
                                            },
                                        });
                                        self.events.push(Event {
                                            name: format!(
                                                "{} {}",
                                                train.train_name, train.train_no
                                            ),
                                            time: stop
                                                .actualDepartureTime
                                                .as_ref()
                                                .map(|_| stop.actualDepartureObject),
                                            planned_time: stop.scheduledDepartureObject,
                                            ty: EventType::Departing,
                                            player: train.t != "bot",
                                            prev: if let (Some(platform), Some(track)) =
                                                (stop.platform.as_ref(), stop.track)
                                            {
                                                format!("{}/{}", platform, track)
                                            } else {
                                                String::from("")
                                            },
                                            next: format!(
                                                "{}/L.{}",
                                                next_stop.nameOfPoint, next_stop.line
                                            ),
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
}

async fn get_servers() -> crate::Result<Vec<Server>> {
    let servers = reqwest::get("https://panel.simrail.eu:8084/servers-open")
        .await?
        .json::<ServerResponse>()
        .await?
        .data;
    Ok(servers)
}
