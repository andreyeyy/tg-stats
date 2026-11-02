use std::process::exit;
use std::io::prelude::*;
use std::io::BufReader;
use std::fs::File;
use std::path::Path;
use std::io::{Seek, SeekFrom};
use std::collections::HashMap;
use serde::Deserialize;

use rev_lines::RevLines;

use tg_export_parser::TgExportParser;

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
    pub join_date: u64,
}
impl User {
    fn new(id: &str, join_date: u64) -> Self {
        User{
            id: id.to_string(),
            name: "Deleted Account".to_string(),
            total_messages: 0,
            total_stickers: 0,
            day_time_distribution: DayTimeDistribution::default(),
            all_time_distribution: [0u64; ALL_TIME_DISTRIBUTION_RESOLUTION],
            join_date,
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

        let parser = TgExportParser::from_file(filename).expect("Could not create parser");

        let mut total_messages: u64 = 0;
        let mut users: HashMap<String, User> = HashMap::new();

        let mut day_time_distribution = DayTimeDistribution::default();
        let mut all_time_distribution = [0u64; ALL_TIME_DISTRIBUTION_RESOLUTION];

        for (i, message) in parser.enumerate() {

            let message = message.unwrap();

            if i == 0 {
                first_message_date = message.date_unixtime.parse().unwrap();
                total_seconds = last_message_date - first_message_date;
            }

            if message.from_id.is_none() {continue;} // service message
            
            // ID of the user
            let id = message.from_id.unwrap();

            // Add user to hash map
            if !users.contains_key(&id) {
                users.insert(id.clone(), User::new(&id, message.date_unixtime.parse().unwrap()));

                // Add the name to the user (if they have one)
                if let Some(username) = message.from {
                    users.get_mut(&id).unwrap().name = username;
                }
            }

            let user = users.get_mut(&id).unwrap();

            // Count total messages of user
            user.total_messages += 1;

            // Total messages in the entire chat
            total_messages += 1;

            // Count stickers
            if message.media_type.is_some() && message.media_type.unwrap() == "sticker"{
                user.total_stickers += 1;
            }

            // Add data to the daily time distributions of both the entire group and the user
            let hour: u64 = (&message.date[11..13]).parse().unwrap();
            *user.day_time_distribution.get_mut(hour as usize).unwrap() += 1;
            *day_time_distribution.get_mut(hour as usize).unwrap() += 1;

            let msg_date: u64 = message.date_unixtime.parse().unwrap();
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


pub fn generate(json_path: &Path) -> Result<ChatStats, StatsError> {
    Ok(ChatStats::from(json_path)?)
}
