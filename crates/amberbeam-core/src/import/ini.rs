//! A small INI reader, because three of the five sources are INI files.
//!
//! Deliberately not a crate. What these files need is: sections in square
//! brackets, `key=value` lines, comments, and leaving the value alone
//! otherwise — no type guessing, no quoting rules, no interpolation. Sixty
//! lines against a dependency that would bring its own opinions about all
//! four.

/// One section, in the order the file had them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub entries: Vec<(String, String)>,
}

impl Section {
    /// The value of a key, compared without regard to case.
    ///
    /// Case-insensitive because these files are written by Windows programs,
    /// where `HostName` and `Hostname` are the same key to everything that
    /// reads them.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
            .map(|(_, value)| value.as_str())
    }

    pub fn number<T: std::str::FromStr>(&self, key: &str) -> Option<T> {
        self.get(key)?.trim().parse().ok()
    }
}

/// Splits text into sections. Anything before the first section header is
/// filed under the empty name, which is where a file without sections ends up.
pub fn parse(text: &str) -> Vec<Section> {
    let mut sections = vec![Section {
        name: String::new(),
        entries: Vec::new(),
    }];

    for line in text.lines() {
        // Only the carriage return of a Windows line ending and whatever sits
        // in front of the key. The end of the line is left alone: a trailing
        // space may be part of a password, and these files are full of
        // passwords.
        let line = line.trim_end_matches('\r').trim_start();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line
            .trim_end()
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            sections.push(Section {
                name: name.trim().to_string(),
                entries: Vec::new(),
            });
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            if let Some(section) = sections.last_mut() {
                // The value is kept as written. A trailing space may be part of
                // a password, and guessing which is which would be worse than
                // either answer.
                section
                    .entries
                    .push((key.trim().to_string(), value.trim_start().to_string()));
            }
        }
    }

    sections.retain(|section| !section.name.is_empty() || !section.entries.is_empty());
    sections
}

/// PuTTY's registry escaping, as WinSCP uses it for names in an INI.
///
/// `%XX` for anything outside the safe set, and a UTF-8 byte-order mark in
/// front when the name needed more than ASCII. Decoding is the whole job:
/// nothing here ever writes one of these files.
pub fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let pair = std::str::from_utf8(&bytes[index + 1..index + 3]).ok();
            if let Some(byte) = pair.and_then(|pair| u8::from_str_radix(pair, 16).ok()) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    super::decode(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_and_keys_come_out_in_order() {
        let text = "\
; a comment
[Sessions\\Kunden/Web]
HostName=example.org
PortNumber=2222

# another comment
[Sessions\\Zweiter]
HostName=zwei.example
";
        let sections = parse(text);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].name, "Sessions\\Kunden/Web");
        assert_eq!(sections[0].get("hostname"), Some("example.org"));
        assert_eq!(sections[0].number::<u16>("PortNumber"), Some(2222));
        assert_eq!(sections[1].get("HostName"), Some("zwei.example"));
    }

    #[test]
    fn a_value_is_kept_exactly_as_written() {
        // An equals sign inside a value belongs to the value, and so does a
        // trailing space: both turn up in passwords.
        let sections = parse("[s]\nPassword=a=b= \nEmpty=");
        assert_eq!(sections[0].get("Password"), Some("a=b= "));
        assert_eq!(sections[0].get("Empty"), Some(""));
    }

    #[test]
    fn escaped_names_come_back() {
        assert_eq!(unescape("my%20session"), "my session");
        assert_eq!(unescape("Kunden/M%C3%BCller"), "Kunden/Müller");
        // A stray per cent that escapes nothing stays a per cent.
        assert_eq!(unescape("100%"), "100%");
        assert_eq!(unescape("a%zz"), "a%zz");
    }
}
