#[macro_use]
extern crate serde_derive;
extern crate itertools;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate log;
extern crate simplelog;

pub mod de;
pub mod model;
pub mod search;

use model::{Day, DayTime, Point, Schedule, Stop, Timestamp};
use std::error::Error;

type Res<T = ()> = Result<T, Box<Error>>;

fn main() -> Res {
    simplelog::TermLogger::init(log::LevelFilter::Debug, Default::default())?;

    let stops = load_stops()?;
    debug!("Loaded {} stops", stops.len());

    let schedules = load_schedules()?;
    debug!("Loaded {} schedules", schedules.len());

    let searcher = search::Searcher::new(stops, schedules);
    debug!("Built searcher");

    let trafi_office = Point { lat: 54.684885, lng: 25.281161 };
    let bus_station = Point { lat: 54.670592, lng: 25.282193 };
    let home = Point { lat: 54.674775, lng: 25.079873 };
    let departure = Timestamp::new(Day::Tuesday, DayTime::new(23, 10));

    info!("Starting route search");
    let route = searcher.find_route(trafi_office, bus_station, departure);
    info!("Finished search, got route? {}", route.is_some());

    if let Some(route) = route {
        println!("Got route");
        for segment in &route.segments {
            println!("{}", segment);
        }
    } else {
        println!("No route found");
    }

    Ok(())
}

fn load_stops() -> Res<Vec<Stop>> {
    let stops_json = std::fs::read_to_string("data/stops.json")?;
    let stops = de::stops(&stops_json)?;
    Ok(stops)
}

fn load_schedules() -> Res<Vec<Schedule>> {
    let schedules_json = std::fs::read_to_string("data/schedules.json")?;
    let schedules = de::schedules(&schedules_json)?;
    Ok(schedules)
}
