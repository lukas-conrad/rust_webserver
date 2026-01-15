use crate::plugin::plugin_config::PluginConfig;
use regex::Regex;
use std::path::Path;

#[derive(Clone)]
pub struct PluginEntry {
    pub config: PluginConfig,
    pub path: Box<Path>,
    host_regex: Vec<Regex>,
    paths_regex: Vec<Regex>,
}

impl PluginEntry {
    pub fn new(config: PluginConfig, path: Box<Path>) -> Self {
        let host_regex = config
            .request_information
            .hosts
            .iter()
            .map(|pattern| {
                let mut regex = String::from("^");
                for ch in pattern.chars() {
                    match ch {
                        '*' => regex.push_str(".*"),
                        '.' => regex.push_str(r"\."),
                        _ => regex.push(ch),
                    }
                }
                regex.push('$');

                Regex::new(&regex).unwrap()
            })
            .collect();
        let paths_regex = config
            .request_information
            .hosts
            .iter()
            .map(|pattern| {
                let mut regex = String::from("^");
                let mut chars = pattern.chars().peekable();

                while let Some(c) = chars.next() {
                    match c {
                        '*' => {
                            if chars.peek() == Some(&'*') {
                                chars.next(); // zweites *
                                regex.push_str(".*");
                            } else {
                                regex.push_str("[^/]+");
                            }
                        }
                        other => {
                            // Regex-Metazeichen escapen
                            if r"\+()[]{}^$|?.".contains(other) {
                                regex.push('\\');
                            }
                            regex.push(other);
                        }
                    }
                }
                regex.push('$');

                Regex::new(&regex).unwrap()
            })
            .collect();

        Self { config, path, host_regex, paths_regex }
    }

    // pub fn match_count() -> u32 {}
}
