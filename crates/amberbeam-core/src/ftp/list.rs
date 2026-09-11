//! Turning what an FTP server says about a directory into rows.
//!
//! Two worlds. `MLSD` is machine readable and dull: key/value pairs and
//! timestamps in UTC, which is why it is preferred whenever `FEAT` mentions it.
//! `LIST` is whatever the server felt like printing, historically a copy of
//! `ls -l`, and that is where the traps are:
//!
//! * A name may contain spaces, so the name is whatever is left after the
//!   known fields — never "the last field".
//! * A date without a year is not this year. In January, a listing showing
//!   "Dec 20" means last December, and a wrong year quietly breaks the
//!   comparison that decides whether a transfer may be resumed.
//! * Windows servers print something else entirely.
//!
//! Everything here is pure text work, tested without a connection, because a
//! parser tested only against one server is a parser that works against one
//! server.

use crate::fs::{DirEntry, EntryKind, Permissions};

/// Days from 1970-01-01 to the given civil date. Howard Hinnant's algorithm,
/// which is the short correct one for the whole Gregorian range.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = month as i64;
    let day = day as i64;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Seconds since the Unix epoch for a UTC date and time.
fn epoch_seconds(year: i64, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> i64 {
    days_from_civil(year, month, day) * 86_400
        + i64::from(hour) * 3600
        + i64::from(minute) * 60
        + i64::from(second)
}

fn month_from_name(name: &str) -> Option<u32> {
    const MONTHS: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let name = name.to_lowercase();
    MONTHS
        .iter()
        .position(|month| name.starts_with(month))
        .map(|index| index as u32 + 1)
}

/// One `MLSD` line: `fact=value;fact=value; name`.
pub fn parse_mlsd_line(line: &str) -> Option<DirEntry> {
    let line = line.trim_end_matches(['\r', '\n']);
    // The name begins after the first space that follows the facts, and may
    // itself contain spaces and semicolons.
    let (facts, name) = line.split_once(' ')?;
    if name.is_empty() {
        return None;
    }

    // A line with no facts at all is not an MLSD line, whatever else it may
    // be. Without this check any sentence with a space in it becomes a file.
    if !facts.contains('=') {
        return None;
    }

    let mut kind = EntryKind::File;
    let mut size = None;
    let mut modified = None;
    let mut permissions = None;
    let mut owner = None;
    let mut group = None;
    let mut link_target = None;

    for fact in facts.split(';').filter(|fact| !fact.is_empty()) {
        let Some((key, value)) = fact.split_once('=') else {
            continue;
        };
        match key.to_lowercase().as_str() {
            "type" => match value.to_lowercase().as_str() {
                "dir" => kind = EntryKind::Directory,
                "file" => kind = EntryKind::File,
                // The current and parent directory are not rows.
                "cdir" | "pdir" => return None,
                other if other.starts_with("os.unix=slink") => {
                    kind = EntryKind::Symlink;
                    // `type=OS.unix=slink:/target` carries the target with it.
                    link_target = value.split_once(':').map(|(_, path)| path.to_string());
                }
                _ => kind = EntryKind::Other,
            },
            "size" => size = value.parse().ok(),
            "modify" => modified = parse_mlsd_time(value),
            "unix.mode" => {
                permissions = u32::from_str_radix(value.trim_start_matches("0o"), 8)
                    .ok()
                    .map(|mode| Permissions(mode & 0o777));
            }
            "unix.ownername" => owner = Some(value.to_string()),
            "unix.groupname" => group = Some(value.to_string()),
            "unix.owner" => owner = owner.or_else(|| Some(value.to_string())),
            "unix.group" => group = group.or_else(|| Some(value.to_string())),
            _ => {}
        }
    }

    Some(DirEntry {
        name: name.to_string(),
        kind,
        size: (kind == EntryKind::File).then_some(size).flatten(),
        modified,
        permissions,
        owner,
        group,
        link_target,
        kind_of_target: None,
    })
}

/// `YYYYMMDDHHMMSS`, always UTC by the specification.
fn parse_mlsd_time(value: &str) -> Option<i64> {
    let digits = value.split('.').next()?;
    if digits.len() < 14 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(epoch_seconds(
        digits[0..4].parse().ok()?,
        digits[4..6].parse().ok()?,
        digits[6..8].parse().ok()?,
        digits[8..10].parse().ok()?,
        digits[10..12].parse().ok()?,
        digits[12..14].parse().ok()?,
    ))
}

/// One `LIST` line, in whichever dialect the server speaks.
///
/// `now` is the current time in seconds, needed because a Unix listing without
/// a year has to be placed in one.
pub fn parse_list_line(line: &str, now: i64) -> Option<DirEntry> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.is_empty() {
        return None;
    }
    if line.starts_with(['-', 'd', 'l', 'b', 'c', 'p', 's']) && line.len() > 10 {
        parse_unix_line(line, now)
    } else {
        parse_windows_line(line)
    }
}

fn parse_unix_line(line: &str, now: i64) -> Option<DirEntry> {
    // Fields are separated by runs of spaces, but the name is not: it is
    // whatever follows the eighth field, spaces and all.
    let mut fields = Vec::new();
    let mut rest = line;
    for _ in 0..8 {
        let trimmed = rest.trim_start();
        let end = trimmed.find(' ')?;
        fields.push(&trimmed[..end]);
        rest = &trimmed[end..];
    }
    let name = rest.trim_start();
    if name.is_empty() {
        return None;
    }

    let mode = fields[0];
    let kind = match mode.as_bytes().first()? {
        b'd' => EntryKind::Directory,
        b'-' => EntryKind::File,
        b'l' => EntryKind::Symlink,
        _ => EntryKind::Other,
    };

    // ` -> ` separates a link from what it points at.
    let (name, link_target) = match (kind, name.split_once(" -> ")) {
        (EntryKind::Symlink, Some((name, target))) => (name, Some(target.to_string())),
        _ => (name, None),
    };

    let size = fields[4].parse::<u64>().ok();
    let modified = parse_unix_time(fields[5], fields[6], fields[7], now);

    Some(DirEntry {
        name: name.to_string(),
        kind,
        size: (kind == EntryKind::File).then_some(size).flatten(),
        modified,
        permissions: parse_mode_letters(mode),
        owner: Some(fields[2].to_string()),
        group: Some(fields[3].to_string()),
        link_target,
        kind_of_target: None,
    })
}

/// `rwxr-xr-x` after the type letter, back into nine bits.
fn parse_mode_letters(mode: &str) -> Option<Permissions> {
    let letters: Vec<char> = mode.chars().skip(1).take(9).collect();
    if letters.len() != 9 {
        return None;
    }
    let mut bits = 0;
    for (index, letter) in letters.iter().enumerate() {
        if *letter != '-' {
            bits |= 1 << (8 - index);
        }
    }
    Some(Permissions(bits))
}

/// `Sep 11 17:02` or `Sep 11 2025`.
///
/// The first form has no year, and assuming the current one is wrong for three
/// weeks every January: a December date seen in January belongs to the year
/// before. A date more than a day in the future is therefore moved back a year.
fn parse_unix_time(month: &str, day: &str, last: &str, now: i64) -> Option<i64> {
    let month = month_from_name(month)?;
    let day = day.parse::<u32>().ok()?;

    if let Some((hour, minute)) = last.split_once(':') {
        let hour = hour.parse().ok()?;
        let minute = minute.parse().ok()?;
        let this_year = year_of(now);
        let candidate = epoch_seconds(this_year, month, day, hour, minute, 0);
        // A day of slack: clocks differ, and a file written a minute ago must
        // not be pushed back a year.
        return Some(if candidate > now + 86_400 {
            epoch_seconds(this_year - 1, month, day, hour, minute, 0)
        } else {
            candidate
        });
    }

    let year = last.parse::<i64>().ok()?;
    Some(epoch_seconds(year, month, day, 0, 0, 0))
}

/// The year a moment falls in, by walking the epoch backwards.
fn year_of(seconds: i64) -> i64 {
    let mut year = 1970 + seconds / 31_556_952;
    // The estimate is at most a year out; two steps settle it.
    for _ in 0..3 {
        let start = epoch_seconds(year, 1, 1, 0, 0, 0);
        let next = epoch_seconds(year + 1, 1, 1, 0, 0, 0);
        if seconds < start {
            year -= 1;
        } else if seconds >= next {
            year += 1;
        } else {
            break;
        }
    }
    year
}

/// `09-11-26  05:02PM       <DIR>          name` — IIS and its imitators.
fn parse_windows_line(line: &str) -> Option<DirEntry> {
    let mut parts = line.split_whitespace();
    let date = parts.next()?;
    let time = parts.next()?;
    let size_or_dir = parts.next()?;

    // The name is the rest of the line after those three fields.
    let after = line.find(size_or_dir)? + size_or_dir.len();
    let name = line[after..].trim_start();
    if name.is_empty() {
        return None;
    }

    let (kind, size) = if size_or_dir.eq_ignore_ascii_case("<DIR>") {
        (EntryKind::Directory, None)
    } else {
        (EntryKind::File, size_or_dir.parse::<u64>().ok())
    };

    Some(DirEntry {
        name: name.to_string(),
        kind,
        size,
        modified: parse_windows_time(date, time),
        // Windows listings carry neither, and an empty column beats an invented
        // value.
        permissions: None,
        owner: None,
        group: None,
        link_target: None,
        kind_of_target: None,
    })
}

/// `09-11-26` and `05:02PM`, in the server's own time zone — which it does not
/// say. Read as UTC, which is the best available guess and the same one every
/// other client makes.
fn parse_windows_time(date: &str, time: &str) -> Option<i64> {
    let mut parts = date.split(['-', '/']);
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    let year = parts.next()?.parse::<i64>().ok()?;
    // Two digit years: 70 and above are the twentieth century, below are this
    // one. The same rule every FTP client uses.
    let year = match year {
        year if year >= 100 => year,
        year if year >= 70 => 1900 + year,
        year => 2000 + year,
    };

    let upper = time.to_uppercase();
    let (clock, shift) = if let Some(rest) = upper.strip_suffix("PM") {
        (rest, 12)
    } else if let Some(rest) = upper.strip_suffix("AM") {
        (rest, 0)
    } else {
        (upper.as_str(), 0)
    };
    let (hour, minute) = clock.split_once(':')?;
    let mut hour: u32 = hour.trim().parse().ok()?;
    let minute: u32 = minute.trim().parse().ok()?;
    if shift == 12 && hour != 12 {
        hour += 12;
    } else if shift == 0 && upper.ends_with("AM") && hour == 12 {
        hour = 0;
    }

    Some(epoch_seconds(year, month, day, hour, minute, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 11 September 2026, 17:02:00 UTC.
    const NOW: i64 = 1_789_146_120;

    #[test]
    fn the_epoch_is_where_it_should_be() {
        assert_eq!(epoch_seconds(1970, 1, 1, 0, 0, 0), 0);
        assert_eq!(epoch_seconds(2000, 1, 1, 0, 0, 0), 946_684_800);
        // A leap day, which is where hand written date code usually breaks.
        assert_eq!(epoch_seconds(2024, 2, 29, 12, 0, 0), 1_709_208_000);
    }

    #[test]
    fn an_mlsd_line_carries_everything() {
        let entry = parse_mlsd_line(
            "type=file;size=4096;modify=20260911170200;UNIX.mode=0644;\
             UNIX.ownername=dennis;UNIX.groupname=staff; index.html",
        )
        .expect("a row");
        assert_eq!(entry.name, "index.html");
        assert_eq!(entry.kind, EntryKind::File);
        assert_eq!(entry.size, Some(4096));
        assert_eq!(entry.modified, Some(1_789_146_120));
        assert_eq!(
            entry.permissions.map(|p| p.to_rwx()),
            Some("rw-r--r--".into())
        );
        assert_eq!(entry.owner.as_deref(), Some("dennis"));
    }

    #[test]
    fn mlsd_names_may_contain_spaces_and_semicolons() {
        let entry = parse_mlsd_line("type=file;size=1; my file; odd.txt").expect("a row");
        assert_eq!(entry.name, "my file; odd.txt");
    }

    #[test]
    fn the_current_and_parent_directory_are_not_rows() {
        assert!(parse_mlsd_line("type=cdir;modify=20260911170200; .").is_none());
        assert!(parse_mlsd_line("type=pdir;modify=20260911170200; ..").is_none());
    }

    #[test]
    fn a_unix_line_is_taken_apart_field_by_field() {
        let entry = parse_list_line(
            "-rw-r--r--   1 dennis   staff        4096 Sep 11 17:02 index.html",
            NOW,
        )
        .expect("a row");
        assert_eq!(entry.name, "index.html");
        assert_eq!(entry.kind, EntryKind::File);
        assert_eq!(entry.size, Some(4096));
        assert_eq!(
            entry.permissions.map(|p| p.to_rwx()),
            Some("rw-r--r--".into())
        );
        assert_eq!(entry.owner.as_deref(), Some("dennis"));
        assert_eq!(entry.group.as_deref(), Some("staff"));
        assert_eq!(entry.modified, Some(1_789_146_120));
    }

    #[test]
    fn a_name_with_spaces_survives() {
        let entry = parse_list_line(
            "-rw-r--r--   1 dennis   staff          12 Sep 11 17:02 Größe & Maß.txt",
            NOW,
        )
        .expect("a row");
        assert_eq!(entry.name, "Größe & Maß.txt");
    }

    #[test]
    fn a_directory_is_recognised_and_has_no_size() {
        let entry = parse_list_line(
            "drwxr-xr-x   2 dennis   staff        4096 Sep 11 17:02 images",
            NOW,
        )
        .expect("a row");
        assert_eq!(entry.kind, EntryKind::Directory);
        assert!(entry.is_directory());
        assert_eq!(entry.size, None);
    }

    #[test]
    fn a_link_is_split_from_what_it_points_at() {
        let entry = parse_list_line(
            "lrwxrwxrwx   1 dennis   staff           7 Sep 11 17:02 current -> releases/7",
            NOW,
        )
        .expect("a row");
        assert_eq!(entry.name, "current");
        assert_eq!(entry.kind, EntryKind::Symlink);
        assert_eq!(entry.link_target.as_deref(), Some("releases/7"));
    }

    #[test]
    fn a_date_with_a_year_instead_of_a_time_is_read() {
        let entry = parse_list_line(
            "-rw-r--r--   1 dennis   staff         512 Jul 14 2019 old.txt",
            NOW,
        )
        .expect("a row");
        assert_eq!(entry.modified, Some(epoch_seconds(2019, 7, 14, 0, 0, 0)));
    }

    #[test]
    fn a_december_date_seen_in_january_belongs_to_the_year_before() {
        // The trap: a listing has no year, and assuming the current one puts a
        // file three weeks into the future — which would then break the check
        // that decides whether a transfer may be resumed.
        let january = epoch_seconds(2027, 1, 5, 12, 0, 0);
        let entry = parse_list_line(
            "-rw-r--r--   1 dennis   staff         512 Dec 20 09:30 late.txt",
            january,
        )
        .expect("a row");
        assert_eq!(entry.modified, Some(epoch_seconds(2026, 12, 20, 9, 30, 0)));
    }

    #[test]
    fn a_file_written_a_moment_ago_stays_in_this_year() {
        let entry = parse_list_line(
            "-rw-r--r--   1 dennis   staff         512 Sep 11 17:02 fresh.txt",
            NOW,
        )
        .expect("a row");
        assert_eq!(entry.modified, Some(epoch_seconds(2026, 9, 11, 17, 2, 0)));
    }

    #[test]
    fn a_size_beyond_four_gigabytes_is_not_truncated() {
        let entry = parse_list_line(
            "-rw-r--r--   1 dennis   staff  8589934592 Sep 11 17:02 big.iso",
            NOW,
        )
        .expect("a row");
        assert_eq!(entry.size, Some(8_589_934_592));
    }

    #[test]
    fn a_windows_listing_is_read_too() {
        let entry =
            parse_list_line("09-11-26  05:02PM             4096 index.html", NOW).expect("a row");
        assert_eq!(entry.name, "index.html");
        assert_eq!(entry.size, Some(4096));
        assert_eq!(entry.modified, Some(epoch_seconds(2026, 9, 11, 17, 2, 0)));
        assert_eq!(entry.permissions, None, "Windows says nothing about modes");
    }

    #[test]
    fn a_windows_directory_says_dir_instead_of_a_size() {
        let entry =
            parse_list_line("09-11-26  05:02PM       <DIR>          images", NOW).expect("a row");
        assert_eq!(entry.kind, EntryKind::Directory);
        assert_eq!(entry.name, "images");
        assert_eq!(entry.size, None);
    }

    #[test]
    fn midnight_and_noon_are_the_two_that_go_wrong() {
        let midnight =
            parse_list_line("09-11-26  12:30AM             10 a.txt", NOW).expect("a row");
        assert_eq!(
            midnight.modified,
            Some(epoch_seconds(2026, 9, 11, 0, 30, 0))
        );
        let noon = parse_list_line("09-11-26  12:30PM             10 b.txt", NOW).expect("a row");
        assert_eq!(noon.modified, Some(epoch_seconds(2026, 9, 11, 12, 30, 0)));
    }

    #[test]
    fn nonsense_is_skipped_rather_than_guessed_at() {
        assert!(parse_list_line("", NOW).is_none());
        assert!(parse_list_line("total 48", NOW).is_none());
        assert!(parse_mlsd_line("no facts here").is_none());
    }
}
