#[macro_use]
extern crate systemd;
extern crate chrono;
extern crate hex;

use chrono::{DateTime, Duration};
use std::collections::HashMap;
use std::string::String;
use std::time;
use std::time::SystemTime;
use std::vec::Vec;
use systemd::id128;
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
    timestamp: SystemTime,
    duration: Duration,
    datetime: chrono::NaiveDateTime,
    message: String,
}

type EventList = Vec<Event>;
struct BootEvents {
    boot_time: chrono::NaiveDateTime,
    boot_id: String,
    events: EventList,
}

fn have_journal() -> bool {
    std::path::Path::new("/run/systemd/journal/").exists()
}

fn ts_to_date(ts: SystemTime) -> chrono::NaiveDateTime {
    let dur = Duration::from_std(ts.duration_since(SystemTime::UNIX_EPOCH).unwrap()).unwrap();
    chrono::NaiveDateTime::from_timestamp(dur.num_seconds(), 0)
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
    j.match_add(BOOTID, bootid);
    j.seek(journal::JournalSeek::Tail);
    j.match_flush().unwrap();
    j.next_record().unwrap()
}

fn discover_previous_boot(j: &mut journal::Journal) -> Option<journal::JournalRecord> {
    let bootid = get_boot_id(j);
    j.match_flush().unwrap();
    j.match_add(BOOTID, bootid);
    j.seek(journal::JournalSeek::Head);
    j.match_flush().unwrap();
    j.previous_record().unwrap()
}
fn print_boots(boots: Vec<BootEvents>) {
    for b in boots.iter() {
        println!("\nboot_id: {:?}", b.boot_id);
        println!("|  Event  |         Date        | Duration |");
        println!("| ------- | ------------------- | -------- |");
        for event in b.events.iter() {
            //if event.duration.num_seconds() != 0 || event.kind == EventType::Boot {
            println!(
                "| {:?} | {} | {} sec | {} |",
                event.kind,
                event.datetime,
                event.duration.num_seconds(),
                event.message
            );
            //}
        }
    }
}

fn get_events_for_day(j: &mut journal::Journal, day: chrono::NaiveDateTime) -> Vec<Event> {
    // we'll store all events here and return it
    let mut events = Vec::<Event>::new();
    j.match_flush().unwrap();

    events
}

fn get_event_for_boot(j: &mut journal::Journal, bootid: String) -> Vec<Event> {
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
        events.push(Event {
            kind: EventType::Boot,
            timestamp: timestamp,
            datetime: ts_to_date(timestamp),
            duration: Duration::zero(),
            message: "Boot [[".to_string() + x.get(MESSAGE).unwrap() + "]]",
        });
    } else {
        println!("warning: nonono");
        return events;
    }

    // adding filters for the relevant messages:
    for msg in MSGS.iter() {
        j.match_add(MSGPREFIX, *msg).unwrap();
        j.match_or().unwrap();
    }

    // Iterate over systemd log
    while let Some(x) = j.next_record().unwrap() {
        //let boot_id = x.get(BOOTID).unwrap().clone();
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

        events.push(if let Some(last) = events.last() {
            Event {
                kind: _type,
                timestamp: timestamp,
                datetime: ts_to_date(timestamp),
                duration: Duration::from_std(timestamp.duration_since(last.timestamp).unwrap())
                    .unwrap(),
                message: message.to_string(),
            }
        } else {
            // Actually this should never happen due to the boot event being present?
            Event {
                kind: _type,
                timestamp: timestamp,
                datetime: ts_to_date(timestamp),
                duration: Duration::zero(),
                message: message.to_string(),
            }
        })
    }

    return events;
}

fn collect_bootids(j: &mut journal::Journal, from: usize, to: usize) -> Vec<String> {
    let mut bootids = Vec::<String>::new();

    // seek to end first
    j.seek(journal::JournalSeek::Tail).unwrap();

    let mut i = 0;
    while i <= to {
        let prev_msg = discover_previous_boot(j).unwrap();
        let prev_boot_id = prev_msg.get(BOOTID).unwrap().clone();

        if i >= from {
            bootids.push(prev_boot_id);
        }

        i += 1;
    }

    bootids
}

fn summarize_daily(boots: BootEvents) {

    // TODO: current data structure stores events per-boot
}

fn main() {
    if !have_journal() {
        println!("systemd-journald doesn't seem to available!");
        return;
    }

    //let current_boot_only = true;
    let mut j = journal::Journal::open(journal::JournalFiles::System, false, true).unwrap();
    let current_boot = get_current_boot_id();
    println!("current boot {:?}", current_boot);

    j.seek(journal::JournalSeek::Tail).unwrap();
    let prev_msg = discover_previous_boot(&mut j).unwrap();
    let prev_boot_id = prev_msg.get(BOOTID).unwrap().clone();
    println!("prev boot {:?}", current_boot);

    //let search_boot_id = current_boot;
    let search_boot_id = prev_boot_id;

    let mut boots: Vec<BootEvents> = Vec::new();

    for bootid in collect_bootids(&mut j, 1, 2) {
        let events = get_event_for_boot(&mut j, bootid.clone());
        boots.push(BootEvents {
            boot_id: bootid,
            boot_time: events.first().unwrap().datetime,
            events: events,
        });
    }

    print_boots(boots);

    // TODO:
    // * fix shit
    // * format/print as table
    // * daily/weekly summary
    // * CLI args
    //
}
