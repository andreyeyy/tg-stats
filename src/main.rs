use std::env;
use std::process::exit;
use std::io::prelude::*;
use std::io::BufReader;
use std::fs::File;
use std::collections::HashMap;
use serde::Deserialize;

mod visualize;

type TimeDistribution = [u64; 24];

#[derive (Debug)]
struct User {
    name: String,
    id: String,
    total_messages: u64,
    total_stickers: u64,
    time_distribution: TimeDistribution,
}
impl User {
    fn new(id: &str) -> Self {
        User{
            id: id.to_string(),
            name: "Deleted Account".to_string(),
            total_messages: 0,
            total_stickers: 0,
            time_distribution: TimeDistribution::default(),
        }
    }
}

#[derive (Debug)]
struct ChatStats {
    name: String,
    total_messages: u64,
    time_distribution: TimeDistribution,
    users: HashMap<String, User>,
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
    fn from(filename: &str) -> ChatStats{
        let f = File::open(filename).unwrap_or_else(|_err|{
            eprintln!("Cannot open file: {}", filename);
            exit(1);
        });

        let reader = BufReader::new(f);
        let parser = JsonArrayParser::new(reader);

        let mut total_messages: u64 = 0;
        let mut users: HashMap<String, User> = HashMap::new();
        let mut time_distribution = TimeDistribution::default();

        for buf in parser {
            let message_str = str::from_utf8(&buf).unwrap();
            let msg = Message::from_json(message_str);

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
            *user.time_distribution.get_mut(hour as usize).unwrap() += 1;
            *time_distribution.get_mut(hour as usize).unwrap() += 1;

        }

        return ChatStats {
            name: "Chat Name".to_string(),
            total_messages,
            users,
            time_distribution,
        }

    }
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

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = match args.get(1) {
        None => {
            eprintln!("No path provided");
            exit(1);
        },
        Some(x) => x,
    };

    let stats = ChatStats::from(path);

    visualize::visualize(&stats);
}
