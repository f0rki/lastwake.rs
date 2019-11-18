extern crate chrono;
// TODO: switch to clap macros?
#[macro_use]
extern crate clap;
extern crate hex;
extern crate systemd;

use chrono::{Date, DateTime, Duration, TimeZone};
use std::string::String;
use std::time::SystemTime;
use std::vec::Vec;
use systemd::journal;

static BOOTID: &'static str = "_BOOT_ID";
static MESSAGE: &'static str = "MESSAGE";

// Generally: Hibernation = to disk; Suspend = to RAM;
//
// TODO: systemd now supports more sleep/hibernation modes (see man systemd.special)
// * hybrid-sleep.target
// * suspend-then-hibernate.target
//
// All those targets pull in the "sleep.target", which we can search for in the journal to get all
// the sleepy targets... However, then we don't know what kind of sleep it was? (except when
// searching for the specific wakup message)
//
// Currently we're taking the start timestamp from the messages that show up when the special
// systemd targets/services are activated.
static SUSPENDSTART: &'static str = "Starting Suspend...";
static HIBERNATESTART: &'static str = "Starting Hibernate...";
static SUSPENDWAKE: &'static str = "ACPI: Waking up from system sleep state S3";
static HIBERNATEWAKE: &'static str = "ACPI: Waking up from system sleep state S4";
static SHUTTINGDOWN: &'static str = "Shutting down.";

static MSGS: [&str; 5] = [
    SUSPENDSTART,
    HIBERNATESTART,
    SUSPENDWAKE,
    HIBERNATEWAKE,
    SHUTTINGDOWN,
];

#[derive(Debug, Eq, PartialEq)]
enum EventType {
    Unknown,
    Boot,
    WakeUp,
    Sleep,
    Shutdown,
}

// TODO: parse the type of sleep from the log and properly store it as enum in the Event struct
/*
enum SleepType {
    Suspend, // S3
    Hibernate, // S4
    // TODO: not sure what kind of ACPI wakeup message we will see with those targets?
    HybridSleep,
    SuspendThenHibernate,
}
*/

#[derive(Debug)]
struct Event {
    kind: EventType,
    timestamp: std::time::SystemTime,
    timestamp_mono: u64,
    // TODO: remove duration from Event and calculate on-the-fly!
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

fn open_journal() -> journal::Journal {
    journal::Journal::open(journal::JournalFiles::System, false, true).unwrap()
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

fn get_first_record_of(
    j: &mut journal::Journal,
    bootid: Option<String>,
) -> Option<journal::JournalRecord> {
    let _bootid: String = if let Some(x) = bootid {
        x
    } else {
        get_boot_id(j)
    };
    j.match_flush().unwrap();
    j.match_add(BOOTID, _bootid).unwrap();
    j.seek(journal::JournalSeek::Head).unwrap();
    j.match_flush().unwrap();
    j.previous_record().unwrap();
    j.next_record().unwrap()
}

fn format_boot_record(j: &mut journal::Journal, x: journal::JournalRecord) -> Event {
    let timestamp = j.timestamp().unwrap();
    let (ts_mono, _) = j.monotonic_timestamp().unwrap();
    let boot_id = x.get(BOOTID).unwrap().clone();
    Event {
        kind: EventType::Boot,
        timestamp: timestamp,
        timestamp_mono: ts_mono,
        datetime: ts_to_date(timestamp),
        duration: Duration::zero(),
        message: "Boot [[".to_string() + x.get(MESSAGE).unwrap() + "]]",
        boot_id: boot_id,
    }
}

fn get_boot_event(j: &mut journal::Journal, bootid: Option<String>) -> Option<Event> {
    // we want to avoid actually changing j, so we open a new journal and seek to the current cursor
    if let Some(x) = get_first_record_of(j, bootid) {
        Some(format_boot_record(j, x))
    } else {
        None
    }
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
    let mut total_waketime = chrono::Duration::seconds(0);
    println!("|    Wake at       |      Sleep at    | Awake For | Type  |");
    println!("| ---------------- | ---------------- | --------- | ----- |");
    //intln!("| 2019-01-01 01:01 | 2019-01-01 01:01 |     00:00 |");
    let mut i = events.iter();
    while let Some(start_event) = i.next() {
        match start_event.kind {
            EventType::Boot | EventType::WakeUp => {
                if let Some(next_event) = i.next() {
                    // we ignore events with less than a second duration
                    if next_event.duration.num_seconds() > 0 {
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

                        total_waketime = total_waketime + next_event.duration;
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
    println!(
        "Total Time Awake: {:02}:{:02}",
        total_waketime.num_hours(),
        (total_waketime - Duration::hours(total_waketime.num_hours())).num_minutes(),
    );
}

fn print_events_boots(boots: Vec<BootEvents>) {
    for b in boots.iter() {
        if let Some(f) = b.events.first() {
            println!("\nboot {} on {}", b.boot_id, f.datetime);
            println!(
                "|  Event   |                 Date              | Monotonic TS | Duration | Message |"
            );
            println!(
                "| -------- | --------------------------------- | ------------ | -------- | ------- |"
            );
            for event in b.events.iter() {
                println!(
                    "| {:8} | {} | {}  |  {:02}:{:02}   | {} |",
                    format!("{:?}", event.kind),
                    event.datetime,
                    event.timestamp_mono,
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
    only_boot_id: Option<String>,
    min_timestamp: Option<DateTime<chrono::Local>>,
    max_timestamp: Option<DateTime<chrono::Local>>,
) {
    let mut current_bootid: String;
    j.match_flush().unwrap();
    if let Some(_bootid) = only_boot_id.clone() {
        // we seek to the start of the boot
        j.match_add(BOOTID, _bootid.clone()).unwrap();
        j.seek(journal::JournalSeek::Head).unwrap();

        // now if we're there anyway, we get the first message of the boot and add it to our events
        // list
        let r = j.next_record().unwrap().unwrap();
        let x = format_boot_record(j, r);
        events.push(x);

        j.match_flush().unwrap();
        j.match_add(BOOTID, _bootid.clone()).unwrap();
        j.match_and().unwrap();

        current_bootid = _bootid.clone();
    } else {
        let x = j.next_record().unwrap().unwrap();
        let bid = x.get(BOOTID).unwrap().clone();
        j.previous_record().unwrap();
        let c = j.cursor().unwrap().clone();
        if let Some(_min_ts) = min_timestamp {
            if let Some(bootevent) = get_boot_event(j, Some(bid.clone())) {
                if bootevent.datetime > _min_ts {
                    events.push(bootevent);
                }
            }
        } else {
            if let Some(bootevent) = get_boot_event(j, Some(bid.clone())) {
                events.push(bootevent);
            }
        }

        j.seek(journal::JournalSeek::Cursor { cursor: c.clone() })
            .unwrap();

        current_bootid = bid;
    }
    // adding filters for the relevant messages:
    for msg in MSGS.iter() {
        j.match_add(MESSAGE, *msg).unwrap();
        if msg != MSGS.last().unwrap() {
            j.match_or().unwrap();
        }
    }

    // Iterate over systemd log
    while let Some(x) = j.next_record().unwrap() {
        let boot_id = x.get(BOOTID).unwrap().clone();
        let message = x.get(MESSAGE).unwrap().clone();
        let timestamp = j.timestamp().unwrap();
        let (ts_mono, _) = j.monotonic_timestamp().unwrap();

        let mut _type = EventType::Unknown;
        if message.contains(SUSPENDSTART) || message.contains(HIBERNATESTART) {
            _type = EventType::Sleep;
        } else if message.contains(SHUTTINGDOWN) {
            _type = EventType::Shutdown;
        } else if message.contains(SUSPENDWAKE) || message.contains(HIBERNATEWAKE) {
            _type = EventType::WakeUp;
        } else {
            panic!("There should never be a unknown event type!");
        }

        if let Some(maxts) = max_timestamp {
            if ts_to_date(timestamp) > maxts {
                if _type == EventType::Sleep || _type == EventType::Shutdown {
                    // TODO: handle the edge case, where the wakeup event was on the day X (X <
                    // maxts), but the sleepevent und the following day.
                    //

                    return;
                } else {
                    return;
                }
            }
        }

        // Here we check if the boot_id changed and inject the boot event.
        // Problem is: there is no clear log line that would allow us to identify the boot.
        // So we basically have to find the first log entry that does have a different boot id to
        // identify boot mesages.
        //
        // Of course this doesn't work if we use the journal match API, because there we need to
        // supply fixed strings...
        //
        // WARNING: this block seems to invalidate some objects returned by systemd...
        // --> solution was to move this whole block after all the information about the current
        // record (in the variable x) is extracted
        if boot_id != current_bootid {
            // backup the current journal cursor
            let c = j.cursor().unwrap().clone();

            // now we get the boot event and put it between events
            // WARNING: get_boot_event uses the match API of journald and therefore breaks the
            // iteration order in the main loop of this function
            if let Some(_min_ts) = min_timestamp {
                if let Some(mut bootevent) = get_boot_event(j, Some(boot_id.clone())) {
                    if bootevent.datetime > _min_ts {
                        if let Some(last) = events.last() {
                            bootevent.duration = Duration::from_std(
                                bootevent.timestamp.duration_since(last.timestamp).unwrap(),
                            )
                            .unwrap()
                        }

                        events.push(bootevent);
                    }
                }
            } else {
                // copy-paste from the block above
                if let Some(mut bootevent) = get_boot_event(j, Some(boot_id.clone())) {
                    if let Some(last) = events.last() {
                        bootevent.duration = Duration::from_std(
                            bootevent.timestamp.duration_since(last.timestamp).unwrap(),
                        )
                        .unwrap()
                    }

                    events.push(bootevent);
                }
            }

            // we flush the matches and seek to the backed up cursor, such that we can take of where
            // we left.
            j.match_flush().unwrap();
            j.seek(journal::JournalSeek::Cursor { cursor: c.clone() })
                .unwrap();

            // we need to re-add the matches here
            j.match_flush().unwrap();
            if let Some(_bootid) = only_boot_id.clone() {
                j.match_add(BOOTID, _bootid.clone()).unwrap();
                j.match_and().unwrap();
            }
            // adding filters for the relevant messages:
            for msg in MSGS.iter() {
                j.match_add(MESSAGE, *msg).unwrap();
                if msg != MSGS.last().unwrap() {
                    j.match_or().unwrap();
                }
            }

            // finally we change the current_bootid to detect the next boot_id change
            current_bootid = boot_id.clone();
        }

        // and now we push the actual event that we found
        // WARNING: do not use any method on the record x or journal j here!
        events.push(if let Some(last) = events.last() {
            Event {
                kind: _type,
                timestamp: timestamp,
                timestamp_mono: ts_mono,
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
                timestamp_mono: ts_mono,
                datetime: ts_to_date(timestamp),
                duration: Duration::zero(),
                message: message.to_string(),
                boot_id: boot_id,
            }
        });
        //println!("{:?}", events.last().unwrap());
    }
}

fn get_events_for_boot(j: &mut journal::Journal, bootid: String) -> Vec<Event> {
    // we'll store all events here and return it
    let mut events = Vec::<Event>::new();
    collect_events(j, &mut events, Some(bootid), None, None);
    return events;
}

fn get_events_for_day(j: &mut journal::Journal, day: Date<chrono::Local>) -> Vec<Event> {
    // we'll store all events here and return it
    let mut events = Vec::<Event>::new();
    j.match_flush().unwrap();

    // seek to 00:00 of the day
    let day_start = day.and_hms(0, 0, 0);
    let ts = day_start.naive_local().timestamp_nanos() / 1000;
    j.match_flush().unwrap();
    j.seek(journal::JournalSeek::ClockRealtime { usec: ts as u64 })
        .unwrap();

    // collect all events until 23:59 of the day
    let end_day = day_start + chrono::Duration::days(1);

    collect_events(j, &mut events, None, Some(day_start), Some(end_day));

    events
}

fn collect_bootids(j: &mut journal::Journal, from: usize, to: usize) -> Vec<String> {
    let mut bootids = Vec::<String>::new();

    // seek to end first
    j.match_flush().unwrap();
    j.seek(journal::JournalSeek::Tail).unwrap();

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

fn parse_ymd(s: &str) -> chrono::NaiveDate {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap_or_else(|e| {
        clap::Error::with_description(
            &format!("Invalid date format; expecting '%Y-%m-%d' ({})", e),
            clap::ErrorKind::ValueValidation,
        )
        .exit()
    })
}

fn main() {
    let app = clap::App::new("lastwake.rs")
        .version("0.1")
        .setting(clap::AppSettings::AllowNegativeNumbers)
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
        .arg(clap::Arg::with_name("to"));
    let matches = app.get_matches();

    if !have_journal() {
        println!("systemd-journald doesn't seem to available!");
        return;
    }

    let mut j = open_journal();

    let mut boots: Vec<BootEvents> = Vec::new();
    let today = chrono::Local::today();
    let now = chrono::Local::now();

    if matches.is_present("daily") {
        let mut days = if matches.is_present("from") {
            if let Ok(start) = value_t!(matches, "from", i64) {
                let end = if matches.is_present("to") {
                    value_t_or_exit!(matches, "to", i64)
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
                    .from_local_date(&parse_ymd(matches.value_of("from").unwrap()))
                    .unwrap();

                let to_date = if matches.is_present("to") {
                    if let Ok(off) = value_t!(matches, "to", i64) {
                        from_date + Duration::days(off)
                    } else {
                        chrono::Local
                            .from_local_date(&parse_ymd(matches.value_of("to").unwrap()))
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
                //days.push(to_date.clone());
                days.reverse();
                days
            }
        } else {
            vec![today]
        };

        days.reverse();

        for day in days {
            if day > today {
                clap::Error::with_description(
                    "Invalid date format: date must not be in the future!",
                    clap::ErrorKind::ValueValidation,
                )
                .exit()
            }
            // TODO: hmm this is wrong, when there was a day with no sleep/wake cycle.
            println!("\nDay {}", day);
            let events = get_events_for_day(&mut j, day);
            //println!("\nDay {}, {}", day.weekday(), day);
            print_events(events);
        }
    } else {
        let bootids = if matches.is_present("from") {
            if let Ok(start) = matches.value_of("from").unwrap().parse::<isize>() {
                let end = if matches.is_present("to") {
                    //matches.value_of("to").unwrap().parse::<isize>().unwrap()
                    value_t_or_exit!(matches, "to", isize)
                } else {
                    start
                };
                collect_bootids(&mut j, start.abs() as usize, end.abs() as usize)
            } else {
                collect_bootids_from_boot(
                    &mut j,
                    matches.value_of("from").unwrap().to_string(),
                    //matches
                    //    .value_of("to")
                    //    .unwrap_or("0")
                    //    .parse::<isize>()
                    //    .unwrap()
                    //    .abs(),
                    value_t!(matches, "to", isize).unwrap_or(0).abs(),
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
