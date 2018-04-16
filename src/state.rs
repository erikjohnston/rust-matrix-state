use std::collections::{HashMap, HashSet};

use sha1::Sha1;
use smallvec::SmallVec;

use auth::{self, Event};
use state_map::{StateMap, WellKnownEmptyKeys};

/// Resolves a list of states to a single state.
pub fn resolve_state(
    state_sets: Vec<&StateMap<String>>,
    event_map: &HashMap<String, Event>,
) -> StateMap<String> {
    if state_sets.len() == 0 {
        return StateMap::new();
    }

    let mut unconflicted = state_sets[0].clone();
    let mut conflicted: StateMap<SmallVec<[&auth::Event; 5]>> = StateMap::new();

    for map in &state_sets[1..] {
        'outer: for ((t, s), eid) in map.iter() {
            if let Some(mut v) = conflicted.get_mut(t, s) {
                for ev in v.iter() {
                    if &ev.event_id == eid {
                        continue 'outer;
                    }
                }
                v.push(&event_map[eid]);
                continue;
            }
            if let Some(eid_prev) = unconflicted.add_or_remove(t, s, eid) {
                let mut v = conflicted.get_mut_or_default(t, s);
                v.push(&event_map[eid]);
                v.push(&event_map[&eid_prev]);
            }
        }
    }

    let mut auth_events_types = HashSet::new();
    for events in conflicted.values() {
        for event in events {
            auth_events_types.extend(auth::auth_types_for_event(event).into_iter())
        }
    }

    let mut auth_events = StateMap::new();
    for (t, s) in auth_events_types {
        if let Some(evid) = unconflicted.get(&t, &s) {
            auth_events.insert(&t, &s, &event_map[evid as &str]);
        }
    }

    let mut resolved_state = unconflicted;

    if let Some(events) = conflicted.get_well_known(WellKnownEmptyKeys::PowerLevels) {
        let ev = resolve_auth_events(
            (WellKnownEmptyKeys::PowerLevels.as_str(), ""),
            events.to_vec(),
            &auth_events,
        );

        resolved_state.insert_well_known(WellKnownEmptyKeys::PowerLevels, ev.event_id.clone());
        auth_events.insert_well_known(WellKnownEmptyKeys::PowerLevels, ev);
    }

    let join_auth_events = auth_events.clone();
    for (state_key, events) in conflicted.iter_join_rules() {
        let key = ("m.room.join_rules", state_key);
        let ev = resolve_auth_events(key, events.to_vec(), &join_auth_events);

        resolved_state.insert(key.0, key.1, ev.event_id.clone());
        auth_events.insert(key.0, key.1, ev);
    }

    let member_auth_events = auth_events.clone();
    for (user, events) in conflicted.iter_members() {
        let key = ("m.room.member", user);
        let ev = resolve_auth_events(key, events.to_vec(), &member_auth_events);

        resolved_state.insert(key.0, key.1, ev.event_id.clone());
        auth_events.insert(key.0, key.1, ev);
    }

    for (key, events) in conflicted.iter_non_members() {
        if !resolved_state.contains_key(key.0, key.1) {
            let ev = resolve_normal_events(events.to_vec(), &auth_events);

            resolved_state.insert(key.0, key.1, ev.event_id.clone());
        }
    }

    resolved_state
}

fn resolve_auth_events<'a>(
    key: (&str, &str),
    mut events: Vec<&'a auth::Event>,
    auth_events: &StateMap<&'a auth::Event>,
) -> &'a auth::Event {
    order_events(&mut events);
    events.reverse();

    let mut new_auth_events = auth_events.clone();

    let key = (&key.0 as &str, &key.1 as &str);

    let mut prev_event = &events[0];
    for event in &events[1..] {
        new_auth_events.insert(key.0, key.1, prev_event);

        if auth::check(event, &new_auth_events).is_err() {
            return prev_event;
        }

        prev_event = event
    }

    return prev_event;
}

fn resolve_normal_events<'a>(
    mut events: Vec<&'a auth::Event>,
    auth_events: &StateMap<&'a auth::Event>,
) -> &'a auth::Event {
    order_events(&mut events);

    for event in &events {
        if auth::check(event, &auth_events).is_ok() {
            return event;
        }
    }

    return events.last().unwrap();
}

fn order_events(events: &mut Vec<&auth::Event>) {
    events.sort_by_key(|e| (-(e.depth as i64), Sha1::from(&e.event_id).hexdigest()))
}

#[test]
fn test_order_events() {
    use serde_json;

    let event1 = auth::Event {
        event_id: "@1:a".to_string(),
        depth: 1,

        etype: String::new(),
        state_key: None,
        prev_events: Vec::new(),
        room_id: String::new(),
        redacts: None,
        sender: String::new(),
        content: serde_json::Map::new(),
    };

    let event2 = auth::Event {
        event_id: "@2:a".to_string(),
        depth: 2,

        etype: String::new(),
        state_key: None,
        prev_events: Vec::new(),
        room_id: String::new(),
        redacts: None,
        sender: String::new(),
        content: serde_json::Map::new(),
    };

    let event3 = auth::Event {
        event_id: "@3:b".to_string(),
        depth: 2,

        etype: String::new(),
        state_key: None,
        prev_events: Vec::new(),
        room_id: String::new(),
        redacts: None,
        sender: String::new(),
        content: serde_json::Map::new(),
    };

    let mut vec = vec![&event1, &event2, &event3];

    order_events(&mut vec);

    assert_eq!(vec[0].event_id, event2.event_id);
    assert_eq!(vec[1].event_id, event3.event_id);
    assert_eq!(vec[2].event_id, event1.event_id);
}
