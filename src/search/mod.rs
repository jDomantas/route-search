use itertools::Itertools;
use model::{
    BusSegment, Day, DayTime, Departure, Point, Route, Schedule, Segment, Stop as MStop, Timestamp,
    Track, WalkSegment, DAYS,
};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

// Max walking distance, in meters.
const MAX_WALK_DISTANCE: f64 = 500.0;

#[derive(Debug, Clone)]
struct Stop {
    name: String,
    loc: Point,
    routes: Vec<StopRoute>,
}

#[derive(Debug, Clone)]
struct StopRoute {
    bus: String,
    next_stop: String,
    departure: Timestamp,
    arrival: Timestamp,
    duration: u64,
}

#[derive(Clone)]
pub struct Searcher {
    stops: HashMap<String, Stop>,
}

struct StopInfo<'a> {
    walk_finish: Option<Timestamp>,
    arrival: Timestamp,
    arriving_segment: Segment<'a>,
    parent: Option<&'a str>,
}

#[derive(Debug, Clone)]
struct HeapItem<'a> {
    departure: Timestamp,
    arrival: Timestamp,
    stop: &'a str,
    parent: Option<&'a str>,
    segment: Segment<'a>,
}

impl<'a> Ord for HeapItem<'a> {
    fn cmp(&self, other: &HeapItem<'a>) -> Ordering {
        let order = self.arrival
            .compare_using_departure(other.arrival, self.departure);
        // we want earliest (smallest) items to come first, so they must be greatest
        order.reverse()
    }
}

impl<'a> PartialOrd for HeapItem<'a> {
    fn partial_cmp(&self, other: &HeapItem<'a>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> PartialEq for HeapItem<'a> {
    fn eq(&self, other: &HeapItem<'a>) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<'a> Eq for HeapItem<'a> {}

impl Searcher {
    pub fn new(stops: Vec<MStop>, schedules: Vec<Schedule>) -> Searcher {
        let stops = stops
            .into_iter()
            .map(|stop| {
                let MStop { id, name, loc } = stop;
                (
                    id,
                    Stop {
                        name,
                        loc,
                        routes: Vec::new(),
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        let mut searcher = Searcher { stops };
        for schedule in schedules {
            searcher.add_schedule(schedule);
        }
        searcher.fix_stops();
        searcher
    }

    fn add_schedule(&mut self, schedule: Schedule) {
        for track in schedule.tracks {
            self.add_track(schedule.name.clone(), track);
        }
    }

    fn add_track(&mut self, name: String, track: Track) {
        for ((ai, a), (bi, b)) in track.stops.iter().enumerate().tuple_windows() {
            let stop = self.stops
                .get_mut(a)
                .expect("schedule refers to non-existing stop");

            for &day in DAYS {
                for tt in &track.timetables {
                    if !tt.works_on_day(day) {
                        continue;
                    }
                    for dep in &tt.departures {
                        match *dep {
                            Departure::Exact(time) => {
                                let stop_time = tt.find_stop_time(ai, time);
                                let next_stop_time = tt.find_stop_time(bi, time);
                                let ride_time = next_stop_time
                                    .raw
                                    .checked_sub(stop_time.raw)
                                    .expect("time subtract underflow");

                                let route = StopRoute {
                                    bus: name.clone(),
                                    next_stop: b.clone(),
                                    departure: Timestamp {
                                        day,
                                        time: stop_time,
                                    },
                                    arrival: Timestamp {
                                        day,
                                        time: next_stop_time,
                                    },
                                    duration: ride_time,
                                };
                                stop.routes.push(route);
                            }
                            Departure::Periodic(_) => {
                                // a wild hack appeared!
                                // ignore periodic departures
                            }
                        }
                    }
                }
            }
        }
    }

    fn fix_stops(&mut self) {
        let mut total_edges = 0;
        for stop in self.stops.values_mut() {
            stop.routes.sort_by_key(|route| route.departure);
            total_edges += stop.routes.len();
        }
        debug!(
            "Built graph: nodes {}, edges: {}",
            self.stops.len(),
            total_edges
        );
    }

    pub fn find_route(&self, from: Point, to: Point, day: Day, time: DayTime) -> Option<Route> {
        let mut times = HashMap::<&str, StopInfo>::new();
        let departure = Timestamp::new(day, time);
        let mut queue = BinaryHeap::new();

        for (name, stop) in &self.stops {
            let distance = from.distance(stop.loc);
            if distance > MAX_WALK_DISTANCE {
                continue;
            }
            let walk_time = walk_time(distance);
            let arrival = departure.offset(walk_time);
            let heap_item = HeapItem {
                departure,
                arrival,
                stop: name,
                parent: None,
                segment: Segment::Walk(WalkSegment {
                    from,
                    to: stop.loc,
                    start: departure.time,
                    duration: walk_time,
                }),
            };
            queue.push(heap_item);
        }

        while let Some(item) = queue.pop() {
            let should_replace = match times.get(item.stop) {
                Some(info) => {
                    item.arrival
                        .compare_using_departure(info.arrival, departure)
                        == Ordering::Less
                }
                None => true,
            };
            if !should_replace {
                continue;
            }
            let reached_stop_at = item.arrival;
            trace!(
                "Reached stop {} ({}) at {}",
                item.stop,
                self.stops[item.stop].name,
                reached_stop_at
            );
            let stop = &self.stops[item.stop];
            let dist_to_end = stop.loc.distance(to);
            let walk_finish = if dist_to_end > MAX_WALK_DISTANCE {
                None
            } else {
                Some(reached_stop_at.offset(walk_time(dist_to_end)))
            };
            times.insert(
                item.stop,
                StopInfo {
                    arrival: reached_stop_at,
                    arriving_segment: item.segment,
                    parent: item.parent,
                    walk_finish,
                },
            );

            // check outgoing bus routes
            for route in &stop.routes {
                if reached_stop_at.is_followed_by(route.departure) {
                    // we can use this route
                    let segment = Segment::Bus(BusSegment {
                        bus: &route.bus,
                        from_stop: &item.stop,
                        to_stop: &route.next_stop,
                        start: route.departure.time,
                        duration: route.duration,
                    });
                    let item = HeapItem {
                        departure,
                        arrival: route.arrival,
                        stop: &route.next_stop,
                        parent: Some(item.stop),
                        segment,
                    };
                    queue.push(item);
                }
            }

            // try to walk to nearby stops
            for (id, next_stop) in &self.stops {
                let distance = stop.loc.distance(next_stop.loc);
                if distance > MAX_WALK_DISTANCE {
                    continue;
                }
                let walk_time = walk_time(distance);
                let next_stop_arrival = reached_stop_at.offset(walk_time);
                let segment = Segment::Walk(WalkSegment {
                    from: stop.loc,
                    to: next_stop.loc,
                    start: reached_stop_at.time,
                    duration: walk_time,
                });
                let item = HeapItem {
                    departure,
                    arrival: next_stop_arrival,
                    stop: id,
                    parent: Some(item.stop),
                    segment,
                };
            }
        }

        let (&final_stop, arrival_time) = times
            .iter()
            .flat_map(|(stop, info)| Some((stop, info.walk_finish?)))
            .min_by(|a, b| a.1.compare_using_departure(b.1, departure))?;

        debug!("Found route, arrived at {}", arrival_time);

        let mut route_segments = Vec::new();
        // Segment of walking from the last stop to the end point.
        route_segments.push(Segment::Walk(WalkSegment {
            from: self.stops[final_stop].loc,
            to,
            start: times[final_stop].arrival.time,
            duration: walk_time(self.stops[final_stop].loc.distance(to)),
        }));

        let mut current = final_stop;
        let departure_time;

        loop {
            let info = times.remove(current).unwrap();
            route_segments.push(info.arriving_segment);
            match info.parent {
                Some(parent) => current = parent,
                None => {
                    // segment of walking from the start point to first stop
                    let stop_pos = self.stops[current].loc;
                    let walk_time = walk_time(from.distance(stop_pos));
                    departure_time = info.arrival.neg_offset(walk_time).time;
                    break;
                }
            }
        }

        route_segments.reverse();

        let mut route = Route {
            segments: route_segments,
            departure_time,
            arrival_time: arrival_time.time,
        };

        self.translate_stop_names(&mut route);
        self.post_process_route(&mut route);

        Some(route)
    }

    fn translate_stop_names<'a>(&'a self, route: &mut Route<'a>) {
        for segment in &mut route.segments {
            match *segment {
                Segment::Walk(_) => {}
                Segment::Bus(ref mut segment) => {
                    segment.from_stop = &self.stops[segment.from_stop].name;
                    segment.to_stop = &self.stops[segment.to_stop].name;
                }
            }
        }
    }

    fn post_process_route(&self, route: &mut Route) {
        // join adjacent bus segments that use the same bus
        route.segments.dedup_by(|b, a| match (a, b) {
            (&mut Segment::Bus(ref mut a), &mut Segment::Bus(ref mut b)) => {
                if a.bus != b.bus {
                    return false;
                }
                if a.start.offset(a.duration) != b.start {
                    return false;
                }
                a.duration += b.duration;
                a.to_stop = b.to_stop;
                true
            }
            _ => false,
        });
    }
}

fn walk_time(distance: f64) -> u64 {
    // in meters per second
    let speed = 4.0 * 1000.0 / 3600.0;
    (distance / speed).ceil() as u64
}
