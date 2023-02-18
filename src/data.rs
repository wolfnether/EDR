use core::time::Duration;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Server {
    #[serde(rename(deserialize = "ServerName"))]
    pub server_name: String,
    #[serde(rename(deserialize = "ServerCode"))]
    pub server_code: String,
    #[serde(rename(deserialize = "IsActive"))]
    pub is_active: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Station {
    #[serde(rename(deserialize = "Name"))]
    pub name: String,
    #[serde(rename(deserialize = "Prefix"))]
    pub prefix: String,
    #[serde(rename(deserialize = "DispatchedBy"))]
    pub dispatched_by: Vec<Player>,
    #[serde(rename(deserialize = "Latititude"))]
    pub latitude: f32,
    #[serde(rename(deserialize = "Longitude"))]
    pub longitude: f32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Player {
    #[serde(rename(deserialize = "SteamId"))]
    pub steam_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SteamPlayers {
    #[serde(rename(deserialize = "data"))]
    pub players: Vec<SteamPlayer>,
}

#[derive(Debug, Deserialize)]
pub struct SteamPlayer {
    #[serde(rename(deserialize = "SteamId"))]
    pub steam_id: String,
    #[serde(rename(deserialize = "SteamInfo"))]
    pub steam_info: Vec<SteamInfo>,
}

#[derive(Debug, Deserialize)]
pub struct SteamInfo {
    pub personaname: String,
}

#[derive(Debug, Deserialize)]
pub struct Train {
    #[serde(rename(deserialize = "TrainData"))]
    pub train_data: TrainData,
    #[serde(rename(deserialize = "Vehicles"))]
    pub vehicles: Vec<String>,
    #[serde(rename(deserialize = "TrainName"))]
    pub train_name: String,
    #[serde(rename(deserialize = "TrainNoLocal"))]
    pub train_no: String,
    #[serde(rename(deserialize = "Type"))]
    pub t: String,
    #[serde(rename(deserialize = "StartStation"))]
    pub start: String,
    #[serde(rename(deserialize = "EndStation"))]
    pub end: String,
    #[serde(skip)]
    pub loc: Option<String>,
}
impl Train {
    pub(crate) fn dist_from(&self, station: &Station) -> f32 {
        const R: f32 = 6371.;

        let lat_a = self.train_data.latitude.to_radians();
        let lat_b = station.latitude.to_radians();

        let d_lat = (self.train_data.latitude - station.latitude).to_radians();
        let d_lon = (self.train_data.longitude - station.longitude).to_radians();

        let a =
            (d_lat / 2.0).sin().powi(2) + lat_a.cos() * lat_b.cos() * (d_lon / 2.0).sin().powi(2);

        R * (2.0 * a.sqrt().asin())
    }
}
#[derive(Debug, Deserialize)]
pub struct TrainData {
    #[serde(rename(deserialize = "ControlledBySteamID"))]
    pub controlled_by_steam_id: Option<String>,

    #[serde(rename(deserialize = "Latititute"))]
    pub latitude: f32,
    #[serde(rename(deserialize = "Longitute"))]
    pub longitude: f32,

    #[serde(rename(deserialize = "SignalInFront"))]
    pub signal_in_front: Option<String>,
    #[serde(rename(deserialize = "DistanceToSignalInFront"))]
    pub distance_to_signal_in_front: f32,
    #[serde(rename(deserialize = "Velocity"))]
    pub velocity: f32,

    #[serde(rename(deserialize = "VDDelayedTimetableIndex"))]
    pub vddelayed_timetable_index: isize,
}

#[derive(Debug, Deserialize)]
pub struct StopDescription {
    pub scheduled_arrival_hour: Option<String>,
    pub scheduled_departure_hour: Option<String>,
    pub station: String,
    layover: Option<String>,
    pub stop_type: Option<String>,
    pub line: String,
}
