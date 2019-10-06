# lastwake.rs

This tool queries the systemd journald for events related to boot/sleep/wake/shutdown and prints a
nice summary of the awake times of the machine:

```
$ lastwake

boot d190acb49b474d5e94c13ae12fd3c04d on 2019-09-05 08:44:22.823888 +02:00
|    Wake at       |      Sleep at    | Awake For | Type  |
| ---------------- | ---------------- | --------- | ----- |
| 2019-09-05 08:44 | 2019-09-05 10:40 |     01:56 | Boot => Sleep |
| 2019-09-05 11:04 | 2019-09-05 12:55 |     01:50 | WakeUp => Sleep |
| 2019-09-05 14:02 | 2019-09-05 15:32 |     01:30 | WakeUp => Sleep |
| 2019-09-05 15:36 | 2019-09-05 16:03 |     00:27 | WakeUp => Sleep |
| 2019-09-05 16:35 | 2019-09-05 18:09 |     01:34 | WakeUp => Sleep |
| 2019-09-06 08:42 | 2019-09-06 12:03 |     03:21 | WakeUp => Sleep |
| 2019-09-06 12:20 | 2019-09-06 15:31 |     03:10 | WakeUp => Sleep |
| 2019-09-07 15:44 | 2019-09-07 16:25 |     00:41 | WakeUp => Sleep |
| 2019-09-07 16:27 | 2019-09-07 17:37 |     01:10 | WakeUp => Shutdown |
```

This tool is my rust port of the [lastwake.py](https://github.com/arigit/lastwake.py) script, with
some additional features I use:

* Relative boot ids: `lastwake -2` (second to last boot, same as with `journalctl -b`)
* Daily mode: `lastwake -d` (summarize per day)
* From-To slice syntax:
    * With boots `lastwake -2 -4` (boots -2, -3 and -4)
    * With days `lastwake -d 2019-10-07 2` or equivalently `lastwake -d 2019-10-07 2019-10-08`

### Known Issues

* Daily mode is buggy as hell...

## Installation

```
cargo install --force --git https://github.com/f0rki/lastwake.rs
```
