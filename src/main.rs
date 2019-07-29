extern crate chrono;
extern crate clap;
extern crate hex;
extern crate systemd;

use chrono::{Date, DateTime, Duration, TimeZone, Timelike};
use std::string::String;
use std::time::SystemTime;
use std::vec::Vec;
use systemd::journal;

// Kernel messages lingo: Hibernation = to disk; Suspend = to RAM;
// Sleep = either hibernation (S4) or suspend (S3)

static BOOTID: &'static str = "_BOOT_ID";
static MESSAGE: &'static str = "MESSAGE";

static MSGPREFIX: &'static str = "MESSAGE";

static SUSPENDSTART: &'static str = "Reached target Sleep.";
static HIBERNATESTART: &'static str = "Suspending system...";
static SUSPENDWAKE: &'static str = "ACPI: Waking up from system sleep state S3";
static HIBERNATEWAKE: &'static str = "ACPI: Waking up from system sleep state S4";
static SHUTTINGDOWN: &'static str = "Shutting down.";
// this one doesn't seem to work...
static MSGS: [&str; 5] = [
    SUSPENDSTART,
    HIBERNATESTART,
    SUSPENDWAKE,
    HIBERNATEWAKE,
    SHUTTINGDOWN,
];

#[derive(Debug, Eq, PartialEq)]
enum EventType {
    Boot,
    WakeUp,
    Sleep,
    Shutdown,
}

#[derive(Debug)]
struct Event {
    kind: EventType,
    timestamp: std::time::SystemTime,
    duration: chrono::Duration,
    datetime: chrono::DateTime<chrono::Local>,
    message: String,
    boot_id: String,
}

type EventList = Vec<Event>;
struct BootEvents {
    boot_time: DateTime<chrono::Local>,
    boot_id: String,
    events: EventList,
}

fn have_journal() -> bool {
    std::path::Path::new("/run/systemd/journal/").exists()
}

fn ts_to_date(ts: SystemTime) -> DateTime<chrono::Local> {
    //let dur = Duration::from_std(ts.duration_since(SystemTime::UNIX_EPOCH).unwrap()).unwrap();
    //let dtnaive = chrono::NaiveDateTime::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos());
    let dur = ts.duration_since(SystemTime::UNIX_EPOCH).unwrap();
    chrono::Local.timestamp(dur.as_secs() as i64, dur.subsec_nanos())
}

fn get_boot_id(j: &mut journal::Journal) -> String {
    j.previous_record().unwrap();
    j.next_record()
        .unwrap()
        .unwrap()
        .get(BOOTID)
        .unwrap()
        .clone()
}

fn get_current_boot_id() -> String {
    let _current_boot = systemd::id128::Id128::from_boot().unwrap();
    hex::encode(_current_boot.as_bytes())
}

fn discover_next_boot(j: &mut journal::Journal) -> Option<journal::JournalRecord> {
    let bootid = get_boot_id(j);
    j.match_flush().unwrap();
    j.match_add(BOOTID, bootid).unwrap();
    j.seek(journal::JournalSeek::Tail).unwrap();
    j.match_flush().unwrap();
    j.next_record().unwrap()
}

fn discover_previous_boot(j: &mut journal::Journal) -> Option<journal::JournalRecord> {
    let bootid = get_boot_id(j);
    j.match_flush().unwrap();
    j.match_add(BOOTID, bootid).unwrap();
    j.seek(journal::JournalSeek::Head).unwrap();
    j.match_flush().unwrap();
    j.previous_record().unwrap()
}

fn print_boots(boots: Vec<BootEvents>) {
    let current_boot = get_current_boot_id();
    for b in boots.iter() {
        if let Some(f) = b.events.first() {
            println!("\nboot {} on {}", b.boot_id, f.datetime);
            println!("|    Wake at       |      Sleep at    | Awake For | Type  |");
            println!("| ---------------- | ---------------- | --------- | ----- |");
            //intln!("| 2019-01-01 01:01 | 2019-01-01 01:01 |     00:00 |");
            let mut i = b.events.iter();
            while let Some(start_event) = i.next() {
                match start_event.kind {
                    EventType::Boot | EventType::WakeUp => {
                        if let Some(next_event) = i.next() {
                            if next_event.duration.num_seconds() != 0 {
                                println!(
                                    "| {} | {} |     {:02}:{:02} | {} => {} |",
                                    start_event.datetime.format("%Y-%m-%d %H:%M"),
                                    next_event.datetime.format("%Y-%m-%d %H:%M"),
                                    next_event.duration.num_hours(),
                                    (next_event.duration
                                        - Duration::hours(next_event.duration.num_hours()))
                                    .num_minutes(),
                                    format!("{:?}", start_event.kind),
                                    format!("{:?}", next_event.kind),
                                );
                            }
                        } else {
                            if b.boot_id == current_boot {
                                let datetime = chrono::Local::now();
                                //let datetime = chrono::NaiveDateTime::now();
                                let duration = datetime - start_event.datetime;
                                println!(
                                    "| {} |  (Still Awake)   |     {:02}:{:02} |",
                                    start_event.datetime.format("%Y-%m-%d %H:%M"),
                                    duration.num_hours(),
                                    (duration - Duration::hours(duration.num_hours()))
                                        .num_minutes(),
                                );
                            } else {

                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn print_events(events: EventList) {
    //let current_boot = get_current_boot_id();

    println!("|    Wake at       |      Sleep at    | Awake For | Type  |");
    println!("| ---------------- | ---------------- | --------- | ----- |");
    //intln!("| 2019-01-01 01:01 | 2019-01-01 01:01 |     00:00 |");
    let mut i = events.iter();
    while let Some(start_event) = i.next() {
        match start_event.kind {
            EventType::Boot | EventType::WakeUp => {
                if let Some(next_event) = i.next() {
                    if next_event.duration.num_seconds() != 0 {
                        println!(
                            "| {} | {} |     {:02}:{:02} | {} => {} |",
                            start_event.datetime.format("%Y-%m-%d %H:%M"),
                            next_event.datetime.format("%Y-%m-%d %H:%M"),
                            next_event.duration.num_hours(),
                            (next_event.duration
                                - Duration::hours(next_event.duration.num_hours()))
                            .num_minutes(),
                            format!("{:?}", start_event.kind),
                            format!("{:?}", next_event.kind),
                        );
                    }
                } else {
                    //if start_event.boot_id == current_boot {
                    let datetime = chrono::Local::now();
                    //let datetime = chrono::NaiveDateTime::now();
                    let duration = datetime - start_event.datetime;
                    println!(
                        "| {} |  (Still Awake)   |     {:02}:{:02} |",
                        start_event.datetime.format("%Y-%m-%d %H:%M"),
                        duration.num_hours(),
                        (duration - Duration::hours(duration.num_hours())).num_minutes(),
                    );
                    //} else {

                    //}
                }
            }
            _ => {}
        }
    }
}

fn print_events_boots(boots: Vec<BootEvents>) {
    for b in boots.iter() {
        if let Some(f) = b.events.first() {
            println!("\nboot {} on {}", b.boot_id, f.datetime);
            println!("|  Event   |             Date           | Duration | Message |");
            println!("| -------- | -------------------------- | -------- | ------- |");
            for event in b.events.iter() {
                println!(
                    "| {:8} | {} |  {:02}:{:02}   | {} |",
                    format!("{:?}", event.kind),
                    event.datetime,
                    event.duration.num_hours(),
                    (event.duration - Duration::hours(event.duration.num_hours())).num_minutes(),
                    event.message
                );
            }
        }
    }
}

fn collect_events(
    j: &mut journal::Journal,
    events: &mut Vec<Event>,
    max_timestamp: Option<DateTime<chrono::Local>>,
) {
    // adding filters for the relevant messages:
    for msg in MSGS.iter() {
        j.match_add(MSGPREFIX, *msg).unwrap();
        j.match_or().unwrap();
    }

    // Iterate over systemd log
    while let Some(x) = j.next_record().unwrap() {
        let boot_id = x.get(BOOTID).unwrap().clone();
        let message = x.get(MESSAGE).unwrap();
        let timestamp = j.timestamp().unwrap();

        let mut _type = EventType::Boot;
        if message.contains(SUSPENDSTART) || message.contains(HIBERNATESTART) {
            _type = EventType::Sleep;
        } else if message.contains(SHUTTINGDOWN) {
            _type = EventType::Shutdown;
        } else if message.contains(SUSPENDWAKE) || message.contains(HIBERNATEWAKE) {
            _type = EventType::WakeUp;
        }

        if let Some(maxts) = max_timestamp {
            if ts_to_date(timestamp) > maxts {
                if _type == EventType::Sleep || _type == EventType::Shutdown {
                    // We just add this to the list even though it's past the max_timestamp s.t., we
                    // have the complete wake/sleep pair
                } else {
                    return;
                }
            }
        }

        events.push(if let Some(last) = events.last() {
            Event {
                kind: _type,
                timestamp: timestamp,
                datetime: ts_to_date(timestamp),
                duration: Duration::from_std(timestamp.duration_since(last.timestamp).unwrap())
                    .unwrap(),
                message: message.to_string(),
                boot_id: boot_id,
            }
        } else {
            // Actually this should never happen due to the boot event being present?
            Event {
                kind: _type,
                timestamp: timestamp,
                datetime: ts_to_date(timestamp),
                duration: Duration::zero(),
                message: message.to_string(),
                boot_id: boot_id,
            }
        })
    }
}

fn get_events_for_boot(j: &mut journal::Journal, bootid: String) -> Vec<Event> {
    // we'll store all events here and return it
    let mut events = Vec::<Event>::new();
    // seek to first record in the journal
    j.match_flush().unwrap();
    j.match_add(BOOTID, bootid).unwrap();
    j.match_and().unwrap();
    j.seek(journal::JournalSeek::Head).unwrap();
    j.previous_record().unwrap();
    // assume this is the time of the boot
    if let Some(x) = j.next_record().unwrap() {
        let timestamp = j.timestamp().unwrap();
        let boot_id = x.get(BOOTID).unwrap().clone();
        events.push(Event {
            kind: EventType::Boot,
            timestamp: timestamp,
            datetime: ts_to_date(timestamp),
            duration: Duration::zero(),
            message: "Boot [[".to_string() + x.get(MESSAGE).unwrap() + "]]",
            boot_id: boot_id,
        });
    } else {
        println!("warning: nonono");
        return events;
    }

    collect_events(j, &mut events, None);

    return events;
}

fn get_events_for_day(j: &mut journal::Journal, day: Date<chrono::Local>) -> Vec<Event> {
    // we'll store all events here and return it
    let mut events = Vec::<Event>::new();
    j.match_flush().unwrap();

    // seek to 00:00 of the day
    let day_start = day.and_hms(0, 0, 0);
    let ts = day_start.timestamp_nanos() / 1000;
    j.seek(journal::JournalSeek::ClockRealtime { usec: ts as u64 })
        .unwrap();

    // collect all events until 23:59 of the day
    let end_day = day_start + chrono::Duration::days(1);

    collect_events(j, &mut events, Some(end_day));

    events
}

fn collect_bootids(j: &mut journal::Journal, from: usize, to: usize) -> Vec<String> {
    let mut bootids = Vec::<String>::new();

    // seek to end first
    j.match_flush().unwrap();
    j.seek(journal::JournalSeek::Tail).unwrap();
    j.previous_record().unwrap();

    let mut msg = j.next_record().unwrap().unwrap();
    let mut i = 0;
    while i <= to {
        let boot_id = msg.get(BOOTID).unwrap().clone();

        if i >= from {
            bootids.push(boot_id);
        }

        msg = discover_previous_boot(j).unwrap();

        i += 1;
    }

    bootids
}

fn collect_bootids_from_boot(
    j: &mut journal::Journal,
    start_boot_id: String,
    to: isize,
) -> Vec<String> {
    let mut bootids = Vec::<String>::new();

    bootids.push(start_boot_id.clone());

    j.seek(journal::JournalSeek::Tail).unwrap();
    j.match_flush().unwrap();
    j.match_add(BOOTID, start_boot_id).unwrap();
    j.seek(journal::JournalSeek::Head).unwrap();
    // now the journal cursor is at the first message of the given boot id

    let mut i = 0;
    while i != to {
        bootids.push(if i < to {
            let msg = discover_next_boot(j).unwrap();
            i += 1;
            msg.get(BOOTID).unwrap().clone()
        } else {
            let msg = discover_previous_boot(j).unwrap();
            i -= 1;
            msg.get(BOOTID).unwrap().clone()
        })
    }

    bootids
}

fn main() {
    let matches = clap::App::new("lastwake.rs")
        .version("0.1")
        .about("systemd journal boot/suspend event analyzer")
        .arg(
            clap::Arg::with_name("daily")
                .short("d")
                .long("daily")
                .help("Summarize events per-day")
                .conflicts_with("event_dump"),
        )
        .arg(
            clap::Arg::with_name("event_dump")
                .short("e")
                .long("event-dump")
                .help("Dump all events for a given boot id")
                .conflicts_with("daily"),
        )
        .arg(clap::Arg::with_name("from"))
        .arg(clap::Arg::with_name("to"))
        .get_matches();

    if !have_journal() {
        println!("systemd-journald doesn't seem to available!");
        return;
    }

    let mut j = journal::Journal::open(journal::JournalFiles::System, false, true).unwrap();

    let mut boots: Vec<BootEvents> = Vec::new();

    if matches.is_present("daily") {
        let today = chrono::Local::today();
        let mut days = if matches.is_present("from") {
            if let Ok(start) = matches.value_of("from").unwrap().parse::<i64>() {
                let end = if matches.is_present("to") {
                    matches.value_of("to").unwrap().parse::<i64>().unwrap()
                } else {
                    start
                };
                let mut days = Vec::new();
                for i in start..(end + 1) {
                    days.push(today - Duration::days(i));
                }
                days
            } else {
                //let from_date =
                //    DateTime::parse_from_str(matches.value_of("from").unwrap(), "%Y-%m-%d")
                //        .unwrap()
                //        .date();
                let from_date: Date<chrono::Local> = chrono::Local
                    .from_local_date(
                        &chrono::NaiveDate::parse_from_str(
                            matches.value_of("from").unwrap(),
                            "%Y-%m-%d",
                        )
                        .unwrap(),
                    )
                    .unwrap();

                let to_date = if matches.is_present("to") {
                    if let Ok(off) = matches.value_of("to").unwrap().parse::<i64>() {
                        from_date + Duration::days(off)
                    } else {
                        chrono::Local
                            .from_local_date(
                                &chrono::NaiveDate::parse_from_str(
                                    matches.value_of("to").unwrap(),
                                    "%Y-%m-%d",
                                )
                                .unwrap(),
                            )
                            .unwrap()
                    }
                } else {
                    from_date.clone() + Duration::days(1)
                };

                let mut days = Vec::new();
                let mut day = from_date.clone();
                while day != to_date {
                    days.push(day);
                    day = day + Duration::days(1);
                }
                days.reverse();
                days
            }
        } else {
            vec![today]
        };

        days.reverse();

        for day in days {
            let events = get_events_for_day(&mut j, day);
            //println!("\nDay {}, {}", day.weekday(), day);
            println!("\nDay {}", day);
            print_events(events);
        }
    } else {
        let bootids = if matches.is_present("from") {
            if let Ok(start) = matches.value_of("from").unwrap().parse::<isize>() {
                let end = if matches.is_present("to") {
                    matches.value_of("to").unwrap().parse::<isize>().unwrap()
                } else {
                    start
                };
                collect_bootids(&mut j, start.abs() as usize, end.abs() as usize)
            } else {
                collect_bootids_from_boot(
                    &mut j,
                    matches.value_of("from").unwrap().to_string(),
                    matches
                        .value_of("to")
                        .unwrap_or("0")
                        .parse::<isize>()
                        .unwrap(),
                )
            }
        } else {
            vec![get_current_boot_id()]
        };
        for bootid in bootids {
            let events = get_events_for_boot(&mut j, bootid.clone());
            boots.push(BootEvents {
                boot_id: bootid,
                boot_time: events.first().unwrap().datetime,
                events: events,
            });
        }
        boots.reverse();

        if matches.is_present("event_dump") {
            print_events_boots(boots);
        } else {
            print_boots(boots);
        }
    }
}
