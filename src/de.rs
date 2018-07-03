use model::{Schedule, Stop};
use serde_json;
use std::error::Error;
use Res;

pub fn stops(json: &str) -> Res<Vec<Stop>> {
    #[derive(Deserialize)]
    struct Wrapper {
        #[serde(rename = "Stops")]
        stops: Vec<Stop>,
    }
    let wrapper: Wrapper = serde_json::from_str(json)?;
    Ok(wrapper.stops)
}

pub fn schedules(json: &str) -> Res<Vec<Schedule>> {
    #[derive(Deserialize)]
    struct Wrapper {
        #[serde(rename = "Schedules")]
        schedules: Vec<Schedule>,
    }

    let wrapper: Wrapper = serde_json::from_str(json)?;
    Ok(wrapper.schedules)
}
