use chrono::{DateTime, FixedOffset, Local, TimeZone};
use failure;
use regex::Regex;

use std::fmt;
use std::str;

pub const DEFAULT_DIST: &str = "UNRELEASED-FIXME-AUTOGENERATED-DEBCARGO";
pub const COMMENT_TEAM_UPLOAD: &str = "  * Team upload.";

pub struct ChangelogEntry {
    pub source: String,
    pub version: String,
    pub distribution: String,
    pub options: String,
    pub maintainer: String,
    pub date: DateTime<FixedOffset>,
    pub items: Vec<String>,
}

pub fn local_now() -> DateTime<FixedOffset> {
    let now = Local::now();
    let offset = now
        .timezone()
        .offset_from_local_datetime(&now.naive_local())
        .unwrap();
    now.with_timezone(&offset)
}

impl fmt::Display for ChangelogEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "{} ({}) {}; {}\n",
            self.source, self.version, self.distribution, self.options
        )?;

        for entry in self.items.iter() {
            writeln!(f, "{}", entry)?;
        }

        writeln!(f, "\n -- {}  {}", self.maintainer, self.date.to_rfc2822())
    }
}

fn line_is_blank(s: &str) -> bool {
    s.chars().all(char::is_whitespace)
}

impl str::FromStr for ChangelogEntry {
    type Err = failure::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut lines = s.lines().collect::<Vec<_>>();
        // see https://manpages.debian.org/testing/dpkg-dev/deb-changelog.5.en.html
        // regexes adapted from /usr/share/perl5/Dpkg/Changelog/Entry/Debian.pm

        let firstline = lines[0];
        let re1 =
            Regex::new(r"(?i)^(\w[-+0-9a-z.]*) \(([^\(\) \t]+)\)((?:\s+[-+0-9a-z.]+)+);(.*?)\s*$")
                .unwrap();
        let matches1 = re1.captures(firstline).unwrap();
        let mut i = 1;
        while line_is_blank(lines[i]) {
            i += 1;
        }
        lines = lines.split_off(i);

        while line_is_blank(lines.last().unwrap()) {
            lines.pop();
        }
        let lastline = lines.pop().unwrap();
        while line_is_blank(lines.last().unwrap()) {
            lines.pop();
        }
        let re2 = Regex::new(r"^ \-\- ((?:.*) <(?:.*)>)  ?(\w.*\S)\s*$").unwrap();
        let matches2 = re2.captures(lastline).unwrap();

        Ok(Self::new(
            matches1[1].to_string(),
            matches1[2].to_string(),
            matches1[3].to_string(),
            matches1[4].to_string(),
            matches2[1].to_string(),
            DateTime::parse_from_rfc2822(&matches2[2])?,
            lines.iter().map(|s| s.to_string()).collect(),
        ))
    }
}

impl ChangelogEntry {
    pub fn new(
        source: String,
        version: String,
        distribution: String,
        options: String,
        maintainer: String,
        date: DateTime<FixedOffset>,
        items: Vec<String>,
    ) -> Self {
        ChangelogEntry {
            source,
            version,
            distribution,
            options,
            maintainer,
            date,
            items,
        }
    }

    pub fn maintainer_name(self: &ChangelogEntry) -> String {
        let re = Regex::new(r"^\s*(\S.*\S)\s*<.*>\s*$").unwrap();
        let matches = re.captures(&self.maintainer).unwrap();
        matches[1].to_string()
    }

    pub fn version_parts(self: &ChangelogEntry) -> (String, String) {
        let re = Regex::new(r"^(.*)-([^-]*)$").unwrap();
        let matches = re.captures(&self.version).unwrap();
        (matches[1].to_string(), matches[2].to_string())
    }

    pub fn deb_version_suffix(self: &ChangelogEntry) -> String {
        let re = Regex::new(r".*-([^-]*)$").unwrap();
        re.captures(&self.version).unwrap()[1].to_string()
    }

    pub fn deb_version_suffix_bump(self: &ChangelogEntry) -> String {
        let suf = self.deb_version_suffix();
        let re = Regex::new(r"^((?:.*\D)?)(\d*)$").unwrap();
        let matches = re.captures(&suf).unwrap();
        if matches[2].is_empty() {
            format!("{}.1", &matches[1])
        } else {
            format!(
                "{}{}",
                &matches[1],
                (matches[2].parse::<u64>().unwrap() + 1)
            )
        }
    }
}

pub struct ChangelogIterator<'a> {
    input: &'a [u8],
    index: usize,
}

impl<'a> ChangelogIterator<'a> {
    pub fn from(input: &'a str) -> ChangelogIterator<'a> {
        ChangelogIterator {
            input: input.as_bytes(),
            index: 0,
        }
    }
}

impl<'a> Iterator for ChangelogIterator<'a> {
    type Item = &'a str;

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        let slice = &self.input[self.index..];
        if slice.is_empty() {
            return None;
        }
        let mut result = slice;
        // ghetto parser; also hack around the fact rust's str doesn't
        // support proper indexing, boo
        for (i, c) in slice.iter().enumerate() {
            if *c != b'\n' {
                continue;
            }
            if i < (slice.len() - 1) && (slice[i + 1] as char).is_whitespace() {
                continue;
            }
            self.index += i + 1;
            result = &slice[..=i];
            break;
        }
        Some(str::from_utf8(&result).unwrap())
    }
}
