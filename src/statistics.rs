use std::process::exit;
use std::io::prelude::*;
use std::io::BufReader;
use std::fs::File;
use std::path::Path;
use std::io::{Seek, SeekFrom};
use std::collections::HashMap;
use serde::Deserialize;

use rev_lines::RevLines;

// every item represents how many messages were sent in that hour
type DayTimeDistribution = [u64; 24];

pub const ALL_TIME_DISTRIBUTION_RESOLUTION: usize = 100;

type AllTimeDistribution = [u64; ALL_TIME_DISTRIBUTION_RESOLUTION];

#[derive (Debug)]
pub struct User {
    pub name: String,
    pub id: String,
    pub total_messages: u64,
    pub total_stickers: u64,
    pub day_time_distribution: DayTimeDistribution,
    pub all_time_distribution: AllTimeDistribution,
}
impl User {
    fn new(id: &str) -> Self {
        User{
            id: id.to_string(),
            name: "Deleted Account".to_string(),
            total_messages: 0,
            total_stickers: 0,
            day_time_distribution: DayTimeDistribution::default(),
            all_time_distribution: [0u64; ALL_TIME_DISTRIBUTION_RESOLUTION],
        }
    }
}

#[derive (Debug)]
pub struct ChatStats {
    pub name: String,
    pub total_messages: u64,
    pub day_time_distribution: DayTimeDistribution,
    pub all_time_distribution: AllTimeDistribution,
    pub users: HashMap<String, User>,
    pub last_message_date: u64,
    pub first_message_date: u64,
}

pub enum StatsError {
    NotJson,
}

#[derive(Debug, Deserialize)]
struct Message {
    id: i64,
    date_unixtime: String,
    date: String,
    from: Option<String>,
    from_id: Option<String>,
    media_type: Option<String>,
}

impl Message {
    fn from_json(json_message: &str) -> Self {
        serde_json::from_str(json_message).unwrap()
    }
}

impl ChatStats {
    fn from(filename: &Path) -> Result<ChatStats, StatsError> {
        let mut f = File::open(filename).unwrap_or_else(|_err|{
            eprintln!("Cannot open file: {:?}", filename);
            exit(1);
        });

        let last_message_date = last_message_date(f.try_clone().unwrap());
        let mut first_message_date: u64 = 0;
        
        // the number of seconds between the first and the last message
        let mut total_seconds: u64 = 0;

        f.rewind().unwrap(); // put file cursor back at the start

        let reader = BufReader::new(f);
        let parser = JsonArrayParser::new(reader);

        let mut total_messages: u64 = 0;
        let mut users: HashMap<String, User> = HashMap::new();

        let mut day_time_distribution = DayTimeDistribution::default();
        let mut all_time_distribution = [0u64; ALL_TIME_DISTRIBUTION_RESOLUTION];

        for (i, buf) in parser.enumerate() {

            let message_str = str::from_utf8(&buf).unwrap();
            let msg = Message::from_json(message_str);

            if i == 0 {
                first_message_date = msg.date_unixtime.parse().unwrap();
                total_seconds = last_message_date - first_message_date;
            }

            if msg.from_id.is_none() {continue;} // service message
            
            // ID of the user
            let id = msg.from_id.unwrap();

            // Add user to hash map
            if !users.contains_key(&id) {
                users.insert(id.clone(), User::new(&id));

                // Add the name to the user (if they have one)
                if let Some(username) = msg.from {
                    users.get_mut(&id).unwrap().name = username;
                }
            }

            let user = users.get_mut(&id).unwrap();

            // Count total messages of user
            user.total_messages += 1;

            // Total messages in the entire chat
            total_messages += 1;

            // Count stickers
            if msg.media_type.is_some() && msg.media_type.unwrap() == "sticker"{
                user.total_stickers += 1;
            }

            // Add data to the daily time distributions of both the entire group and the user
            let hour: u64 = (&msg.date[11..13]).parse().unwrap();
            *user.day_time_distribution.get_mut(hour as usize).unwrap() += 1;
            *day_time_distribution.get_mut(hour as usize).unwrap() += 1;

            let msg_date: u64 = msg.date_unixtime.parse().unwrap();
            let bucket = ((msg_date - first_message_date) * ALL_TIME_DISTRIBUTION_RESOLUTION as u64 - 1) / total_seconds;

            all_time_distribution[bucket as usize] += 1;
            user.all_time_distribution[bucket as usize] += 1;

        }

        return Ok(ChatStats {
            name: "Chat Name".to_string(),
            total_messages,
            users,
            day_time_distribution,
            all_time_distribution,
            last_message_date,
            first_message_date,
        })

    }
}

/// Takes result.json file, returns date_unixtime field of the last message
fn last_message_date(file: File) -> u64 {
    let rev_lines = RevLines::new(file);

    for line_option in rev_lines {
        let line: String = line_option.unwrap();

        if line.contains("\"date_unixtime\"") {

            return line
                .split_once(":")
                .unwrap()
                .1
                .trim()
                .trim_matches(&['"', ','][..])
                .parse()
                .unwrap();
        }
    }
    panic!("No last message date found");
}

/// Custom JSON array parser to avoid loading the entire file in memory
struct JsonArrayParser {
    reader: BufReader<File>,
    bracket_level: i32,
    inside_string: bool,
    last_char_escape: bool,
}

impl JsonArrayParser {
    fn new(mut reader: BufReader<File>) -> Self {
        reader.skip_until(b'[').unwrap();
        JsonArrayParser{
            reader,
            inside_string: false,
            bracket_level: 0,
            last_char_escape: false,
        }
    }
}

impl Iterator for JsonArrayParser {
    type Item = Vec<u8>;

    /// Returns the next message in the .json file
    fn next(&mut self) -> Option<Self::Item>{

        let mut message_buf: Vec<u8> = Vec::new();
        let bytes = (&mut self.reader).bytes();

        for b in bytes {
            let b = b.unwrap();

            message_buf.push(b);

            if self.last_char_escape { // if the character before is '\'
                self.last_char_escape = false;
                assert!(self.inside_string, "Unexpected backslash outside of a string");
                assert!(
                    b == b'\\' || 
                    b == b'\"' || 
                    b == b'/' || 
                    b == b'b' || 
                    b == b'f' || 
                    b == b'n' || 
                    b == b'r' || 
                    b == b't' || 
                    b == b'u',
                    "Unexpected escaped character: \\{}", b as char
                );
                continue;
            }

            if b == b'\\' { // escape character
                self.last_char_escape = true;
            }

            else if b == b'\"' { // the " character, not escaped
                self.inside_string = !self.inside_string;
            }

            else if b == b'{' {
                if self.inside_string {continue;}
                self.bracket_level += 1;
            }

            else if b == b'}' {
                if self.inside_string {continue;}
                self.bracket_level -= 1;
                if self.bracket_level == 0 {
                    let _ = self.reader.skip_until(b',');
                    return Some(message_buf);
                }
            }

        }
        return None;
    }
}

pub fn generate(json_path: &Path) -> Result<ChatStats, StatsError> {
    Ok(ChatStats::from(json_path)?)
}
