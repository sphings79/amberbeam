//! `wcx_ftp.ini`, the FTP plug-in of Total Commander.
//!
//! One section per connection, and a password stored as hex. The method behind
//! it is a small generator run from four fixed starting points: rotate every
//! byte, shuffle the lot, exclusive-or, subtract. None of that is encryption —
//! there is no key and nothing is secret — which is why the same four numbers
//! read it back.

use std::path::Path;

use crate::config::AuthKind;
use crate::endpoint::Protocol;
use crate::error::Result;
use crate::ftp::Encryption;

use super::ini;
use super::{Found, Imported, Source};

pub fn read(path: &Path) -> Result<Found> {
    let text = super::read_text(path)?;
    Ok(parse(&text, path))
}

pub fn parse(text: &str, path: &Path) -> Found {
    let mut found = Found::new(Source::WcxFtp, path);

    for section in ini::parse(text) {
        // The file keeps its own settings in sections of a known name; every
        // other section is a connection.
        if section.name.is_empty() || section.name.eq_ignore_ascii_case("default") {
            continue;
        }
        let Some(raw_host) = section.get("host") else {
            continue;
        };

        // The host may carry a port and a starting directory: `example.org:2121/var/www`.
        let (address, from_path) = match raw_host.split_once('/') {
            Some((address, rest)) => (address, Some(format!("/{rest}"))),
            None => (raw_host, None),
        };
        let (host, port) = match address.rsplit_once(':') {
            Some((host, port)) => (host, port.parse::<u16>().ok()),
            None => (address, None),
        };
        if host.is_empty() {
            continue;
        }

        // The plug-in writes `1` for a connection it secures. Which of the two
        // kinds is not recorded, and explicit is the one worth trying first:
        // it is what every server built in the last fifteen years offers.
        let secure = section.get("ssl").map(str::trim) == Some("1")
            || section.get("usetls").map(str::trim) == Some("1");

        let mut entry = Imported::named(&section.name);
        entry.protocol = if secure {
            Protocol::Ftps
        } else {
            Protocol::Ftp
        };
        entry.encryption = secure.then_some(Encryption::Explicit);
        entry.host = host.to_string();
        entry.port = port.unwrap_or(21);
        entry.user = section.get("username").unwrap_or_default().to_string();
        entry.auth = AuthKind::Password;
        entry.remote_path = section
            .get("remotedir")
            .map(str::to_string)
            .filter(|value| !value.is_empty())
            .or(from_path);

        let password = section.get("password").and_then(decode);
        found.entries.push(entry.with_password(password));
    }

    found
}

/// The generator the method is built on: a linear congruence, as plain as they
/// come, run from a known starting point.
struct Generator(u32);

impl Generator {
    const MULTIPLIER: u64 = 0x0808_8405;

    fn below(&mut self, limit: u32) -> u32 {
        self.0 = (u64::from(self.0)
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(1)
            & 0xFFFF_FFFF) as u32;
        ((u64::from(self.0).wrapping_mul(u64::from(limit))) >> 32) as u32
    }
}

/// Turns a stored password back into the password.
///
/// Four passes in reverse order of nothing in particular: the four starting
/// points are constants of the method, not secrets. The last four bytes are a
/// check the method appends and this reader does not verify — a wrong value
/// therefore yields bytes rather than an error, which is why the result is only
/// accepted when it reads as text.
fn decode(stored: &str) -> Option<String> {
    let hex: String = stored.chars().filter(|c| !c.is_whitespace()).collect();
    if hex.is_empty() || hex.len() % 2 != 0 {
        return None;
    }
    let mut bytes: Vec<u8> = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(text, 16).ok()
        })
        .collect::<Option<Vec<u8>>>()?;

    // The last four are the check, not the password.
    let length = bytes.len().checked_sub(4).filter(|&n| n > 0)?;

    let mut rotate = Generator(849_521);
    for byte in bytes.iter_mut().take(length) {
        *byte = byte.rotate_left(rotate.below(8));
    }

    let mut shuffle = Generator(12_345);
    for _ in 0..256 {
        let a = shuffle.below(length as u32) as usize;
        let b = shuffle.below(length as u32) as usize;
        bytes.swap(a, b);
    }

    let mut mask = Generator(42_340);
    for byte in bytes.iter_mut().take(length) {
        *byte ^= mask.below(256) as u8;
    }

    let mut shift = Generator(54_321);
    for byte in bytes.iter_mut().take(length) {
        *byte = byte.wrapping_sub(shift.below(256) as u8);
    }

    bytes.truncate(length);
    // A password is text. Bytes that are not say the value was not a password
    // of this kind, and inventing characters for them would hand somebody a
    // string that silently fails to log in.
    let text = String::from_utf8(bytes).ok()?;
    (!text.is_empty() && !text.chars().any(|c| c.is_control())).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The same four passes forwards, for the tests only.
    ///
    /// Proves the reader matches the published method. That the published
    /// method is right is what a real file proves, and only that.
    fn encode(password: &str) -> String {
        let mut bytes = password.as_bytes().to_vec();
        let length = bytes.len();

        let mut shift = Generator(54_321);
        let shifts: Vec<u8> = (0..length).map(|_| shift.below(256) as u8).collect();
        for (byte, add) in bytes.iter_mut().zip(&shifts) {
            *byte = byte.wrapping_add(*add);
        }

        let mut mask = Generator(42_340);
        let masks: Vec<u8> = (0..length).map(|_| mask.below(256) as u8).collect();
        for (byte, m) in bytes.iter_mut().zip(&masks) {
            *byte ^= m;
        }

        // The shuffle is its own inverse when the swaps are replayed backwards.
        let mut shuffle = Generator(12_345);
        let swaps: Vec<(usize, usize)> = (0..256)
            .map(|_| {
                (
                    shuffle.below(length as u32) as usize,
                    shuffle.below(length as u32) as usize,
                )
            })
            .collect();
        for &(a, b) in swaps.iter().rev() {
            bytes.swap(a, b);
        }

        let mut rotate = Generator(849_521);
        let turns: Vec<u32> = (0..length).map(|_| rotate.below(8)).collect();
        for (byte, turn) in bytes.iter_mut().zip(&turns) {
            *byte = byte.rotate_right(*turn);
        }

        bytes.extend_from_slice(&[0, 0, 0, 0]);
        bytes.iter().map(|byte| format!("{byte:02X}")).collect()
    }

    #[test]
    fn a_stored_password_comes_back() {
        for password in ["tannenbaum", "x", "Größe&Maß", "a rather longer one, 30+"] {
            assert_eq!(decode(&encode(password)).as_deref(), Some(password));
        }
    }

    #[test]
    fn nonsense_decodes_to_nothing_rather_than_to_rubbish() {
        assert_eq!(decode(""), None);
        assert_eq!(decode("ABC"), None, "an odd number of characters");
        assert_eq!(decode("zzzz"), None, "not hex");
        assert_eq!(decode("00000000"), None, "nothing but the check");
    }

    #[test]
    fn one_section_becomes_one_connection() {
        let text = format!(
            "[Kunde Müller]\n\
             host=example.org:2121/var/www\n\
             username=dennis\n\
             password={}\n\
             [default]\n\
             host=ignoriert\n",
            encode("tannenbaum")
        );
        let found = parse(&text, &PathBuf::from("wcx_ftp.ini"));

        assert_eq!(found.entries.len(), 1, "the defaults are not a connection");
        let entry = &found.entries[0];
        assert_eq!(entry.name, "Kunde Müller");
        assert_eq!(entry.host, "example.org");
        assert_eq!(entry.port, 2121);
        assert_eq!(entry.remote_path.as_deref(), Some("/var/www"));
        assert_eq!(entry.protocol, Protocol::Ftp);
        assert_eq!(entry.password.as_deref(), Some("tannenbaum"));
    }

    #[test]
    fn a_secured_connection_is_read_as_explicit() {
        let found = parse(
            "[Web]\nhost=example.org\nusername=dennis\nssl=1\n",
            &PathBuf::from("wcx_ftp.ini"),
        );
        assert_eq!(found.entries[0].protocol, Protocol::Ftps);
        assert_eq!(found.entries[0].encryption, Some(Encryption::Explicit));
    }
}
