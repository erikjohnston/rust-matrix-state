#[macro_use]
extern crate serde_derive;
extern crate heapsize;
extern crate indicatif;
extern crate postgres;
extern crate procinfo;
extern crate serde;
extern crate serde_json;
extern crate sha1;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate heapsize_derive;
extern crate smallvec;
#[macro_use]
extern crate clap;

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::mem;
use std::time::Instant;

use clap::{App, Arg};
use heapsize::HeapSizeOf;
use indicatif::ProgressBar;

pub mod auth;
pub mod state;
pub mod state_map;

use state_map::StateMap;

fn main() {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .arg(Arg::with_name("input")
            .help("File containing the room events one per line")
            .index(1)
            .required(true))
        .arg(Arg::with_name("postgres-connection")
            .help("Postgres connection string")
            .index(2)
            .required(false))
        .get_matches();

    let file_path = value_t_or_exit!(matches, "input", String);
    let pg_conn_str = matches.value_of("postgres-connection");

    let f = File::open(file_path).unwrap();
    let f = BufReader::new(f);

    let mut event_map = HashMap::new(); // event_id -> event
    let mut parents = HashMap::new();  // event_id -> list of parent event_ids
    let mut extremities = HashSet::new();  // set of forward extremities
    let mut roots = Vec::new(); // set of events that have no children

    let start = Instant::now();

    // Read in the events and add them to event_map and co.
    for line in f.lines() {
        let line = line.unwrap();
        let event: auth::Event = serde_json::from_str(&line).unwrap();

        for eid in event.prev_events.iter().map(|v| v.0.clone()) {
            extremities.remove(&eid);
            parents
                .entry(eid)
                .or_insert_with(HashSet::new)
                .insert(event.event_id.clone());
        }

        if !parents.contains_key(&event.event_id) {
            extremities.insert(event.event_id.clone());
        }

        if event.prev_events.is_empty() {
            roots.push(event.event_id.clone());
        }

        event_map.insert(event.event_id.clone(), event);
    }

    println!(
        "Reading took {}",
        indicatif::HumanDuration(Instant::now() - start)
    );

    println!("Missing:");

    for (r, _) in &parents {
        if !event_map.contains_key(r) {
            println!("\t{}", r);
        }
    }

    println!("Extremities:");

    for e in &extremities {
        println!("\t{}", e);
    }

    println!("Roots:");
    for e in &roots {
        println!("\t{}", e);
    }

    let start = Instant::now();

    // Get a list of events in topological order
    let ordered = get_ordered_fast(&event_map, &extremities, &parents);

    println!(
        "Ordering took {}",
        indicatif::HumanDuration(Instant::now() - start)
    );

    let statm = procinfo::pid::statm_self().unwrap();
    println!("{}", indicatif::HumanBytes(statm.resident as u64 * 4096));

    // We now calculate the state of each event

    let pb = ProgressBar::new(ordered.len() as u64);

    let mut next_sg = 0;

    // Multiple events may share the same state, so lets give the state an ID
    // called "state group" and have two maps for event_id -> sg -> state
    let mut event_to_sg = HashMap::new();
    let mut sg_to_state = HashMap::new();

    let start = Instant::now();

    let mut i = 0;
    for eid in &ordered {
        let event = &event_map[eid];

        // The block returns the new state group if a new one was created.
        let state = {
            // Whether the state is the same as a previous state group.
            let mut current_sg = None;

            // Work out the resolved state for all prev_events
            let mut state: Cow<StateMap<_>> = if event.prev_events.len() > 1 {
                let state_sets = event
                    .prev_events
                    .iter()
                    .map(|v| &v.0)
                    .filter_map(|pid| {
                        if let Some(sg) = event_to_sg.get(pid) {
                            if let Some(state) = sg_to_state.get(sg) {
                                Some(state)
                            } else {
                                panic!("Failed to find state for event: {}, {}", pid, eid);
                            }
                        } else {
                            // panic!("Failed to find sg for event: {}, processing: {}", pid, eid);
                            // println!("Ignoring event: {}", pid);
                            None
                        }
                    })
                    .collect();

                Cow::Owned(state::resolve_state(state_sets, &event_map))
            } else if event.prev_events.len() == 1 {
                let s = event_to_sg[&event.prev_events[0].0];
                current_sg = Some(s);
                Cow::Borrowed(&sg_to_state[&s])
            } else {
                Cow::Owned(StateMap::new())
            };

            // If this is a state event then we add it to the state
            if let Some(ref state_key) = event.state_key {
                current_sg = None;
                state.to_mut().insert(&event.etype, &state_key, eid.clone());
            }

            // If nothing has changed we reuse the state group, otherwise
            // create a new one.
            if let Some(sg) = current_sg {
                event_to_sg.insert(eid.clone(), sg);
                None
            } else {
                let sg = next_sg + 1;
                next_sg += 1;

                event_to_sg.insert(eid.clone(), sg);
                Some((sg, state.into_owned()))
            }
        };

        // If we generated a new state group, persist it.
        if let Some((sg, state)) = state {
            sg_to_state.insert(sg, state);
        }

        // Increment progress bar occaisonally (doing it on each loop is slow)
        i += 1;
        if i % 20 == 0 {
            pb.inc(20);
        }
    }

    pb.finish();

    println!(
        "State calculation took {}",
        indicatif::HumanDuration(Instant::now() - start)
    );

    println!("{}", sg_to_state.len());

    println!(
        "Size: {}",
        indicatif::HumanBytes(event_to_sg.heap_size_of_children() as u64)
    );
    println!(
        "Size: {}",
        indicatif::HumanBytes(sg_to_state.heap_size_of_children() as u64)
    );

    let statm = procinfo::pid::statm_self().unwrap();
    println!("{}", indicatif::HumanBytes(statm.resident as u64 * 4096));

    // If we have a db connection, lets see what the difference is between what we
    // think the state is and what the db thinks it is.
    if let Some(pg_conn_str) = pg_conn_str {
        let conn = postgres::Connection::connect(
            pg_conn_str,
            postgres::TlsMode::None,
        ).unwrap();

        // First, lets do a binary search for the first place our views diverge
        let res = ordered.binary_search_by(|event_id| {
            let state: HashSet<_> = sg_to_state[&event_to_sg[event_id]]
                .values()
                .cloned()
                .collect();

            let actual = get_state(&conn, event_id);

            if state == actual {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        });

        let i = res.unwrap_err();

        if i < ordered.len() {
            println!("\nFirst divergence: {} at {}", &ordered[i], i);

            print_difference(
                &ordered[i],
                &conn,
                &event_to_sg,
                &sg_to_state,
                &event_map,
            );
        }

        // Now output the difference for each extremity.
        for e in &extremities {
            println!("\nDifference at extremity {}", e);

            print_difference(e, &conn, &event_to_sg, &sg_to_state, &event_map);
        }
    }

    // Leak these large objects, as their deallocation take a bit of time and
    // we're about to exit...
    mem::forget(sg_to_state);
    mem::forget(event_map);
    mem::forget(event_to_sg);
    mem::forget(ordered);
    mem::forget(parents);
}

fn get_state(conn: &postgres::Connection, event_id: &str) -> HashSet<String> {
    let q = conn.query(GET_STATE_QUERY, &[&event_id]).unwrap();

    q.iter().map(|row| row.get(0)).collect()
}

const GET_STATE_QUERY: &str = r#"
    WITH RECURSIVE state(state_group) AS (
        SELECT state_group FROM event_to_state_groups WHERE event_id = $1
        UNION ALL
        SELECT prev_state_group FROM state_group_edges e, state s
        WHERE s.state_group = e.state_group
    )
    SELECT DISTINCT last_value(event_id) OVER (
        PARTITION BY type, state_key ORDER BY state_group ASC
        ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING
    ) AS event_id FROM state_groups_state
    WHERE state_group IN (
        SELECT state_group FROM state
    )
"#;

/// Return list of events in topological ordering, with root first.
fn get_ordered_fast(
    event_map: &HashMap<String, auth::Event>,
    extremities: &HashSet<String>,
    parents: &HashMap<String, HashSet<String>>,
) -> Vec<String> {
    let mut ordered = Vec::with_capacity(event_map.len());

    let mut zeroes: Vec<_> = extremities.iter().collect();
    let mut adjacents: HashMap<&str, usize> = event_map
        .keys()
        .map(|key| (key as &str, parents.get(key).map(HashSet::len).unwrap_or(0)))
        .collect();

    while let Some(event_id) = zeroes.pop() {
        ordered.push(event_id.clone());

        for (p, _) in &event_map[event_id].prev_events {
            let a = if let Some(mut i) = adjacents.get_mut(p as &str) {
                *i -= 1;
                *i
            } else {
                1
            };

            if a == 0 {
                adjacents.remove(p as &str);
                zeroes.push(p);
            }
        }
    }

    assert_eq!(ordered.len(), event_map.len());

    ordered.reverse();
    ordered
}

fn print_difference(
    event_id: &str,
    conn: &postgres::Connection,
    event_to_state: &HashMap<String, i32>,
    sg_to_state: &HashMap<i32, StateMap<String>>,
    event_map: &HashMap<String, auth::Event>,
) {
    let actual = get_state(&conn, event_id);
    let state: HashSet<_> = sg_to_state[&event_to_state[event_id]]
        .values()
        .cloned()
        .collect();

    let mut difference = false;
    for e in actual.symmetric_difference(&state) {
        difference = true;

        let event = &event_map[e];
        if actual.contains(e) {
            println!(
                " - ({}, {}) {}",
                event.etype,
                event.state_key.as_ref().unwrap(),
                e
            );
        } else {
            println!(
                " + ({}, {}) {}",
                event.etype,
                event.state_key.as_ref().unwrap(),
                e
            );
        }
    }

    if !difference {
        println!(" No difference");
    }
}
