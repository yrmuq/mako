// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const SEARCH_DELAY: Duration = Duration::from_millis(400);
const REFRESH: Duration = Duration::from_secs(3600);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Dry,
    Rain,
    Snow,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Location {
    pub id: u32,
    pub name: String,
    pub country: String,
    #[serde(default)]
    pub admin1: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
}

impl Location {
    pub fn valid(&self) -> bool {
        let valid_text = |text: &str| {
            !text.trim().is_empty() && text.len() <= 240 && !text.chars().any(char::is_control)
        };
        self.id != 0
            && valid_text(&self.name)
            && valid_text(&self.country)
            && self.admin1.as_deref().is_none_or(valid_text)
            && (-90.0..=90.0).contains(&self.latitude)
            && (-180.0..=180.0).contains(&self.longitude)
    }

    pub fn label(&self) -> String {
        match self.admin1.as_deref() {
            Some(region) if region != self.name && region != self.country => {
                format!("{}, {}, {}", self.name, region, self.country)
            }
            _ => format!("{}, {}", self.name, self.country),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Idle,
    Searching,
    NoMatches,
    SearchFailed,
    Loading,
    Ready,
    WeatherFailed,
}

enum Update {
    Search(u64, Result<Vec<Location>, ()>),
    Forecast(u64, Result<Kind, ()>),
}

pub struct Weather {
    enabled: bool,
    location: Option<Location>,
    query: String,
    matches: Vec<Location>,
    editing: bool,
    search_status: Status,
    forecast_status: Status,
    kind: Kind,
    search_due: Option<Instant>,
    forecast_due: Option<Instant>,
    search_revision: u64,
    forecast_revision: u64,
    searching: bool,
    fetching: bool,
    tx: Sender<Update>,
    rx: Receiver<Update>,
}

impl Weather {
    pub fn new(location: Option<Location>, enabled: bool) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut weather = Self {
            enabled: false,
            location,
            query: String::new(),
            matches: Vec::new(),
            editing: false,
            search_status: Status::Idle,
            forecast_status: Status::Idle,
            kind: Kind::Dry,
            search_due: None,
            forecast_due: None,
            search_revision: 0,
            forecast_revision: 0,
            searching: false,
            fetching: false,
            tx,
            rx,
        };
        weather.reset_search();
        weather.set_enabled(enabled);
        weather
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled != enabled {
            self.enabled = enabled;
            self.reset_search();
            self.restart_forecast();
        }
    }

    pub fn edit(&mut self, query: String) {
        if !self.enabled || query == self.query {
            return;
        }
        self.query = query.chars().take(120).collect();
        self.editing = true;
        self.matches.clear();
        self.search_revision += 1;
        let valid = self.query.trim().chars().count() >= 3;
        self.search_status = if valid {
            Status::Searching
        } else {
            Status::Idle
        };
        self.search_due = valid.then(|| Instant::now() + SEARCH_DELAY);
    }

    pub fn select(&mut self, index: usize) -> Option<Location> {
        if !self.enabled {
            return None;
        }
        let place = self.matches.get(index)?.clone();
        self.location = Some(place.clone());
        self.reset_search();
        self.restart_forecast();
        Some(place)
    }

    fn reset_search(&mut self) {
        self.query = self.location.as_ref().map_or_else(String::new, |place| {
            format!("{}, {}", place.name, place.country)
        });
        self.editing = false;
        self.matches.clear();
        self.search_status = Status::Idle;
        self.search_due = None;
        self.search_revision += 1;
    }

    fn restart_forecast(&mut self) {
        self.forecast_revision += 1;
        self.kind = Kind::Dry;
        self.forecast_due = (self.enabled && self.location.is_some()).then(Instant::now);
        self.forecast_status = if self.forecast_due.is_some() {
            Status::Loading
        } else {
            Status::Idle
        };
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn matches(&self) -> &[Location] {
        &self.matches
    }

    pub fn status(&self) -> Status {
        if self.editing {
            self.search_status
        } else {
            self.forecast_status
        }
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn tick(&mut self, language: impl FnOnce() -> String) -> bool {
        let mut changed = false;
        while let Ok(update) = self.rx.try_recv() {
            match update {
                Update::Search(revision, result) => {
                    self.searching = false;
                    if self.enabled && revision == self.search_revision {
                        match result {
                            Ok(matches) => {
                                self.search_status = if matches.is_empty() {
                                    Status::NoMatches
                                } else {
                                    Status::Idle
                                };
                                self.matches = matches;
                            }
                            Err(()) => {
                                eprintln!("MAKO-WEATHER-0001: Could not load city suggestions from Open-Meteo");
                                self.search_status = Status::SearchFailed;
                            }
                        }
                        changed = true;
                    }
                }
                Update::Forecast(revision, result) => {
                    self.fetching = false;
                    if self.enabled && revision == self.forecast_revision {
                        self.forecast_due = Some(Instant::now() + REFRESH);
                        match result {
                            Ok(kind) => {
                                self.kind = kind;
                                self.forecast_status = Status::Ready;
                            }
                            Err(()) => {
                                eprintln!("MAKO-WEATHER-0002: Could not load current precipitation from Open-Meteo");
                                self.forecast_status = Status::WeatherFailed;
                                self.kind = Kind::Dry;
                            }
                        }
                        changed = true;
                    }
                }
            }
        }
        if !self.enabled {
            return changed;
        }
        let now = Instant::now();
        if !self.searching && self.search_due.is_some_and(|at| now >= at) {
            let query = self.query.trim().to_owned();
            let language = language();
            let revision = self.search_revision;
            let tx = self.tx.clone();
            self.search_due = None;
            self.searching = true;
            std::thread::spawn(move || {
                let _ = tx.send(Update::Search(revision, search(&query, &language)));
            });
        }
        if !self.fetching && self.forecast_due.is_some_and(|at| now >= at) {
            if let Some(place) = self.location.clone() {
                let revision = self.forecast_revision;
                let tx = self.tx.clone();
                self.forecast_due = None;
                self.fetching = true;
                std::thread::spawn(move || {
                    let _ = tx.send(Update::Forecast(revision, forecast(&place)));
                });
            }
        }
        changed
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(12)))
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into()
}

fn search(query: &str, language: &str) -> Result<Vec<Location>, ()> {
    let (city, country) = query
        .split_once(',')
        .map_or((query, ""), |(city, country)| (city.trim(), country.trim()));
    if city.trim().chars().count() < 2 {
        return Ok(Vec::new());
    }
    let country_code = country.len() == 2 && country.bytes().all(|byte| byte.is_ascii_alphabetic());
    let language = if country.is_empty() {
        language
    } else if country
        .chars()
        .any(|ch| ('\u{0400}'..='\u{052f}').contains(&ch))
    {
        "ru"
    } else {
        "en"
    };
    let mut request = agent()
        .get("https://geocoding-api.open-meteo.com/v1/search")
        .query("name", city)
        .query("language", language)
        .query(
            "count",
            if country.is_empty() || country_code {
                "4"
            } else {
                "100"
            },
        )
        .header("User-Agent", concat!("mako/", env!("CARGO_PKG_VERSION")));
    if country_code {
        request = request.query("countryCode", country.to_ascii_uppercase());
    }
    let body = request
        .call()
        .map_err(|_| ())?
        .body_mut()
        .with_config()
        .limit(96 * 1024)
        .read_to_string()
        .map_err(|_| ())?;
    parse_locations(&body, if country_code { "" } else { country })
}

fn parse_locations(body: &str, qualifier: &str) -> Result<Vec<Location>, ()> {
    #[derive(Deserialize)]
    struct Match {
        #[serde(flatten)]
        location: Option<Location>,
    }
    #[derive(Deserialize)]
    struct Places {
        #[serde(default)]
        results: Vec<Match>,
    }
    let suggestions: Places = serde_json::from_str(body).map_err(|_| ())?;
    let qualifier = qualifier.to_lowercase();
    let qualifier = qualifier.strip_prefix("the ").unwrap_or(&qualifier);
    let matches = |text: &str| {
        let normalized = text.to_lowercase();
        normalized
            .strip_prefix("the ")
            .unwrap_or(&normalized)
            .starts_with(qualifier)
    };
    Ok(suggestions
        .results
        .into_iter()
        .filter_map(|item| item.location)
        .filter(Location::valid)
        .filter(|place| matches(&place.country) || place.admin1.as_deref().is_some_and(matches))
        .take(4)
        .collect())
}

#[derive(Deserialize)]
struct Current {
    rain: f32,
    showers: f32,
    snowfall: f32,
}

impl Current {
    fn kind(self) -> Kind {
        if self.snowfall > 0.0 {
            Kind::Snow
        } else if self.rain > 0.0 || self.showers > 0.0 {
            Kind::Rain
        } else {
            Kind::Dry
        }
    }
}

fn forecast(place: &Location) -> Result<Kind, ()> {
    #[derive(Deserialize)]
    struct Conditions {
        current: Current,
    }
    let body = agent()
        .get("https://api.open-meteo.com/v1/forecast")
        .query("latitude", place.latitude.to_string())
        .query("longitude", place.longitude.to_string())
        .query("current", "rain,showers,snowfall")
        .header("User-Agent", concat!("mako/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|_| ())?
        .body_mut()
        .with_config()
        .limit(32 * 1024)
        .read_to_string()
        .map_err(|_| ())?;
    let conditions: Conditions = serde_json::from_str(&body).map_err(|_| ())?;
    Ok(conditions.current.kind())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn city_results_keep_valid_disambiguated_places() {
        let places = parse_locations(
            r#"{"results":[
                {"id":1,"name":"Delft","admin1":"South Holland","country":"The Netherlands","latitude":52.01,"longitude":4.35},
                {"id":2,"name":"Invalid","country":"Test","latitude":100,"longitude":4},
                {"id":3,"name":"Missing country","latitude":58,"longitude":4}
            ]}"#,
            "Netherlands",
        ).unwrap();
        assert_eq!(places.len(), 1);
        assert_eq!(places[0].label(), "Delft, South Holland, The Netherlands");
        assert!(parse_locations("{}", "").unwrap().is_empty());
    }

    #[test]
    fn forecast_data_distinguishes_rain_showers_and_snow() {
        for (rain, showers, snowfall, expected) in [
            (0.0, 0.0, 0.0, Kind::Dry),
            (0.2, 0.0, 0.0, Kind::Rain),
            (0.0, 0.2, 0.0, Kind::Rain),
            (0.2, 0.0, 0.3, Kind::Snow),
        ] {
            assert_eq!(
                Current {
                    rain,
                    showers,
                    snowfall
                }
                .kind(),
                expected
            );
        }
    }

    #[test]
    fn an_old_forecast_cannot_override_a_new_location_or_reenable_weather() {
        let mut state = Weather::new(None, true);
        let tx = state.tx.clone();
        let old = state.forecast_revision;
        state.set_enabled(false);
        state.set_enabled(true);
        tx.send(Update::Forecast(old, Ok(Kind::Rain))).unwrap();
        assert!(!state.tick(|| "en".into()));
        assert_eq!(state.kind(), Kind::Dry);
        assert_eq!(state.status(), Status::Idle);
    }
}
