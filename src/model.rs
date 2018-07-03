use serde;
use std::cmp::Ordering;
use std::fmt;

#[derive(Deserialize, Debug, Copy, Clone)]
pub struct Point {
    #[serde(rename = "Lat")]
    pub lat: f64,
    #[serde(rename = "Lng")]
    pub lng: f64,
}

impl Point {
    /// Distance between two points in meters.
    pub fn distance(&self, other: Point) -> f64 {
        fn radians(deg: f64) -> f64 {
            deg / 180.0 * ::std::f64::consts::PI
        }

        let r = 6371e3;
        let phi1 = radians(self.lat);
        let phi2 = radians(other.lat);
        let delta_phi = radians(other.lat - self.lat);
        let delta_lambda = radians(other.lng - self.lng);
        let a = (delta_phi / 2.0).sin().powi(2)
            + phi1.cos() * phi2.cos() * (delta_lambda / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        c * r
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Stop {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(flatten)]
    pub loc: Point,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Schedule {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "LongName")]
    pub long_name: String,
    #[serde(rename = "Tracks")]
    pub tracks: Vec<Track>,
    #[serde(rename = "TransportId")]
    pub transport_type: TransportType,
}

#[derive(Deserialize, PartialEq, Eq, Debug, Copy, Clone)]
pub enum TransportType {
    #[serde(rename = "vln_trol")]
    Trolley,
    #[serde(rename = "vln_bus")]
    Bus,
    #[serde(rename = "vln_expressbus")]
    Express,
    #[serde(rename = "vln_nightbus")]
    NightBus,
}

impl fmt::Display for TransportType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TransportType::Trolley => write!(f, "trolley"),
            TransportType::Bus | TransportType::Express | TransportType::NightBus => {
                write!(f, "bus")
            }
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Track {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Stops", deserialize_with = "de_stop_ids")]
    pub stops: Vec<String>,
    #[serde(rename = "Timetables")]
    pub timetables: Vec<Timetable>,
}

fn de_stop_ids<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct IdWrapper {
        #[serde(rename = "StopId")]
        id: String,
    }
    let wrapper: Vec<IdWrapper> = serde::Deserialize::deserialize(deserializer)?;
    Ok(wrapper.into_iter().map(|w| w.id).collect())
}

#[derive(Deserialize, Debug, Clone)]
pub struct Timetable {
    #[serde(rename = "Days")]
    pub days: u8,
    #[serde(rename = "Departures")]
    pub departures: Vec<Departure>,
    #[serde(rename = "StopDurations")]
    pub durations: Vec<Durations>,
}

impl Timetable {
    pub fn find_stop_time(&self, index: usize, dep: DayTime) -> DayTime {
        let durations = &self.durations[index];
        for entry in &durations.entries {
            if entry.from <= dep && dep < entry.to {
                let ride_time = entry.time;
                return DayTime {
                    raw: dep.raw + ride_time,
                };
            }
        }
        panic!("Cannot find stop time");
    }
}

impl Timetable {
    pub fn works_on_day(&self, day: Day) -> bool {
        let flag = 1 << day.index();
        (self.days | flag) != 0
    }
}

#[derive(Deserialize, Debug, Copy, Clone)]
#[serde(untagged)]
pub enum Departure {
    Exact(DayTime),
    Periodic(Periodic),
}

#[derive(Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug, Copy, Clone)]
pub struct DayTime {
    #[serde(rename = "Time")]
    pub raw: u64,
}

impl DayTime {
    pub fn new(hours: u64, minutes: u64) -> DayTime {
        assert!(hours < 24, "hours should be in range [0; 23]");
        assert!(minutes < 60, "minutes should be in range [0; 59]");
        DayTime {
            raw: hours * 3600 + minutes * 60,
        }
    }

    pub fn offset(&self, offset: u64) -> DayTime {
        DayTime {
            raw: self.raw + offset,
        }
    }

    pub fn neg_offset(&self, offset: u64) -> DayTime {
        DayTime {
            raw: self.raw - offset,
        }
    }
}

impl fmt::Display for DayTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let minutes = self.raw / 60 % 60;
        let hours = {
            let h = self.raw / 3600 % 24;
            if h == 0 && minutes == 0 {
                24
            } else {
                h
            }
        };
        write!(f, "{:02}:{:02}", hours, minutes)
    }
}

#[derive(Deserialize, Debug, Copy, Clone)]
pub struct Periodic {
    #[serde(rename = "FromTime", deserialize_with = "de_day_time")]
    from: DayTime,
    #[serde(rename = "ToTime", deserialize_with = "de_day_time")]
    to: DayTime,
}

fn de_day_time<'de, D>(deserializer: D) -> Result<DayTime, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = serde::Deserialize::deserialize(deserializer)?;
    Ok(DayTime { raw })
}

#[derive(Deserialize, Debug, Clone)]
pub struct Durations {
    #[serde(rename = "Durations")]
    pub entries: Vec<Entry>,
}

#[derive(Deserialize, Debug, Copy, Clone)]
pub struct Entry {
    #[serde(rename = "FromTime", deserialize_with = "de_day_time")]
    pub from: DayTime,
    #[serde(rename = "ToTime", deserialize_with = "de_day_time")]
    pub to: DayTime,
    #[serde(rename = "Duration")]
    pub time: u64,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Copy, Clone)]
pub enum Day {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Day {
    pub fn index(&self) -> u8 {
        match *self {
            Day::Sunday => 0,
            Day::Monday => 1,
            Day::Tuesday => 2,
            Day::Wednesday => 3,
            Day::Thursday => 4,
            Day::Friday => 5,
            Day::Saturday => 6,
        }
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match *self {
            Day::Monday => "Mon",
            Day::Tuesday => "Tue",
            Day::Wednesday => "Wed",
            Day::Thursday => "Thu",
            Day::Friday => "Fri",
            Day::Saturday => "Sat",
            Day::Sunday => "Sun",
        };
        write!(f, "{}", name)
    }
}

pub const DAYS: &[Day] = &[
    Day::Monday,
    Day::Tuesday,
    Day::Wednesday,
    Day::Thursday,
    Day::Friday,
    Day::Saturday,
    Day::Sunday,
];

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct Timestamp {
    pub day: Day,
    pub time: DayTime,
}

impl Timestamp {
    pub fn new(day: Day, time: DayTime) -> Timestamp {
        Timestamp { day, time }
    }

    pub fn offset(&self, offset: u64) -> Timestamp {
        Timestamp {
            day: self.day,
            time: self.time.offset(offset),
        }
    }

    pub fn neg_offset(&self, offset: u64) -> Timestamp {
        Timestamp {
            day: self.day,
            time: self.time.neg_offset(offset),
        }
    }

    pub fn compare_using_departure(&self, other: Timestamp, departure: Timestamp) -> Ordering {
        if *self == other {
            Ordering::Equal
        } else if *self == departure {
            Ordering::Less
        } else if other == departure {
            Ordering::Greater
        } else if *self < other && other < departure {
            Ordering::Less
        } else if other < departure && departure < *self {
            Ordering::Less
        } else if departure < *self && *self < other {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    }

    // Returns if other timestamp is ahead of this one, but at most a few days.
    pub fn is_followed_by(&self, other: Timestamp) -> bool {
        let departure_day = match self.day {
            Day::Monday => Day::Wednesday,
            Day::Tuesday => Day::Thursday,
            Day::Wednesday => Day::Friday,
            Day::Thursday => Day::Saturday,
            Day::Friday => Day::Sunday,
            Day::Saturday => Day::Monday,
            Day::Sunday => Day::Tuesday,
        };
        let departure = Timestamp::new(departure_day, DayTime::new(0, 0));
        self.compare_using_departure(other, departure) != Ordering::Greater
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.day, self.time)
    }
}

#[derive(Debug, Clone)]
pub struct Route<'a> {
    pub segments: Vec<Segment<'a>>,
    pub departure_time: DayTime,
    pub arrival_time: DayTime,
}

#[derive(Debug, Copy, Clone)]
pub enum Segment<'a> {
    Walk(WalkSegment<'a>),
    Bus(BusSegment<'a>),
}

#[derive(Debug, Copy, Clone)]
pub struct WalkSegment<'a> {
    pub from: NamedPoint<'a>,
    pub to: NamedPoint<'a>,
    pub start: DayTime,
    pub duration: u64,
}

#[derive(Debug, Copy, Clone)]
pub struct NamedPoint<'a> {
    pub loc: Point,
    pub name: Option<&'a str>,
}

impl<'a> fmt::Display for NamedPoint<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(name) = self.name {
            write!(f, "{}", name)
        } else {
            write!(f, "({}; {})", self.loc.lat, self.loc.lng)
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct BusSegment<'a> {
    pub bus: &'a str,
    pub typ: TransportType,
    pub from_stop: &'a str,
    pub to_stop: &'a str,
    pub start: DayTime,
    pub duration: u64,
}

impl<'a> fmt::Display for Segment<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Segment::Walk(ref seg) => write!(f, "{}", seg),
            Segment::Bus(ref seg) => write!(f, "{}", seg),
        }
    }
}

impl<'a> fmt::Display for WalkSegment<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "At {} - walk from {} to {}, walking time: {} minutes",
            self.start,
            self.from,
            self.to,
            (self.duration + 30) / 60,
        )
    }
}

impl<'a> fmt::Display for BusSegment<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "At {} - take {} {} from {} to {}, ride time: {} minutes",
            self.start,
            self.typ,
            self.bus,
            self.from_stop,
            self.to_stop,
            (self.duration + 30) / 60,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_distance() {
        // two random points in Vilnius, with distance measured in Google Maps
        let p1 = Point {
            lat: 54.690740,
            lng: 25.241002,
        };
        let p2 = Point {
            lat: 54.701723,
            lng: 25.264866,
        };
        let distance = p1.distance(p2);
        assert!((distance - 1960.0).abs() < 5.0);
    }
}
