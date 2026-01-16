use crate::plugin::plugin_config::PluginConfig;
use regex::Regex;
use std::cmp::max;
use std::path::Path;

#[derive(Clone)]
pub struct PluginEntry {
    pub config: PluginConfig,
    pub path: Box<Path>,
    host_regex: Vec<Regex>,
    paths_regex: Vec<(Regex, i32)>,
}

impl PluginEntry {
    pub fn new(config: PluginConfig, path: Box<Path>) -> Self {
        let host_regex = Self::create_host_regex(&config.request_information.hosts);
        let paths_regex = Self::create_paths_regex(&config.request_information.paths);

        Self {
            config,
            path,
            host_regex,
            paths_regex,
        }
    }

    fn create_host_regex(config: &[String]) -> Vec<Regex> {
        let host_regex = config
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
        host_regex
    }

    fn create_paths_regex(config: &[String]) -> Vec<(Regex, i32)> {
        config
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

                (
                    Regex::new(&regex).unwrap(),
                    Self::calculate_path_regex_specificity(pattern),
                )
            })
            .collect()
    }

    // TODO: Document specificity
    fn calculate_path_regex_specificity(pattern: &str) -> i32 {
        let mut score = 0;
        let mut chars = pattern.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '*' => {
                    if chars.peek() == Some(&'*') {
                        chars.next();
                        score -= 10;
                    } else {
                        score -= 5;
                    }
                }
                '/' | '\\' => score += 3,
                '.' => score += 2,
                _ => score += 1,
            }
        }

        score
    }

    pub fn match_count(&self, host: &String, path: &String, method: &String) -> u32 {
        let methods = &self.config.request_information.request_methods;
        if !(methods.contains(&"*".to_string()) || methods.contains(method)) {
            return 0;
        }
        let hosts_match = self.host_regex.iter().any(|regex| regex.is_match(host));
        if !hosts_match {
            return 0;
        }
        let path_specificity = self
            .paths_regex
            .iter()
            .map(|item| {
                let (regex, specificity) = item;
                let matches = regex.is_match(path);
                return if matches { specificity.clone() } else { 0 };
            })
            .max();

        if let Some(count) = path_specificity {
            max(count, 0) as u32
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::plugin::plugin_entry::PluginEntry;

    #[test]
    fn simple_test_paths_regex() {
        let vec = PluginEntry::create_paths_regex(&["test/gay/hello.txt".to_string()]);

        assert_eq!(vec.len(), 1);
        assert!(vec[0].0.is_match("test/gay/hello.txt"));
        assert!(!vec[0].0.is_match("/test/gay/hello.txt"));
        assert!(!vec[0].0.is_match("test*/gay/hello.txt"));
        assert!(!vec[0].0.is_match("test/gay/hello"));
    }
    #[test]
    fn test_star_paths_regex() {
        let vec = PluginEntry::create_paths_regex(&["test/*/hello.txt".to_string()]);

        assert_eq!(vec.len(), 1);
        assert!(vec[0].0.is_match("test/gay/hello.txt"));
        assert!(!vec[0].0.is_match("/test/gay/hello.txt"));
        assert!(!vec[0].0.is_match("test*/gay/hello.txt"));
        assert!(!vec[0].0.is_match("test/gay/hello"));
        assert!(vec[0].0.is_match("test/haloael/hello.txt"));
    }
    #[test]
    fn test_double_star_paths_regex() {
        let vec = PluginEntry::create_paths_regex(&["test/**/hello.*".to_string()]);

        assert_eq!(vec.len(), 1);
        assert!(vec[0].0.is_match("test/gay/hello.txt"));
        assert!(!vec[0].0.is_match("/test/gay/hello.txt"));
        assert!(!vec[0].0.is_match("test*/gay/hello.txt"));
        assert!(!vec[0].0.is_match("test/gay/hello"));
        assert!(vec[0].0.is_match("test/haloael/hello.txt"));
        assert!(vec[0].0.is_match("test/haloael/how/are/you/hello.txt"));
        assert!(vec[0].0.is_match("test/haloael/how/are/you/hello.wtf"));
    }

    #[test]
    fn simple_test_host_regex() {
        let vec = PluginEntry::create_host_regex(&["www.your.mom.de".to_string()]);

        assert_eq!(vec.len(), 1);
        assert!(vec[0].is_match("www.your.mom.de"));
        assert!(!vec[0].is_match("www.hello.your.mom.de"));
        assert!(!vec[0].is_match("www.your-mom.de"));
        assert!(!vec[0].is_match("your.mom"));
    }
    #[test]
    fn test_star_host_regex() {
        let vec = PluginEntry::create_host_regex(&["www.*.mom.de".to_string()]);

        assert_eq!(vec.len(), 1);
        assert!(vec[0].is_match("www.your.mom.de"));
        assert!(vec[0].is_match("www.hello.your.mom.de"));
        assert!(vec[0].is_match("www.xd.mom.de"));
        assert!(!vec[0].is_match("www.your-mom.de"));
    }

    #[test]
    fn test_path_specificity_exact_path() {
        // Exact paths should have high specificity
        let specificity = PluginEntry::calculate_path_regex_specificity("api/users/profile.json");
        assert_eq!(specificity, 27);
    }

    #[test]
    fn test_path_specificity_with_single_wildcard() {
        // Single wildcard reduces specificity
        let specificity = PluginEntry::calculate_path_regex_specificity("api/*/profile.json");
        assert_eq!(specificity, 17);

        // Should be less specific than exact path
        let exact_specificity = PluginEntry::calculate_path_regex_specificity("api/users/profile.json");
        assert!(specificity < exact_specificity);
    }

    #[test]
    fn test_path_specificity_with_double_wildcard() {
        // Double wildcard reduces specificity even more
        let specificity = PluginEntry::calculate_path_regex_specificity("api/**/profile.json");
        assert_eq!(specificity, 12);

        // Should be less specific than single wildcard
        let single_wildcard = PluginEntry::calculate_path_regex_specificity("api/*/profile.json");
        assert!(specificity < single_wildcard);
    }

    #[test]
    fn test_path_specificity_comparison() {
        // Compare different pattern types
        let pattern1 = "static/css/main.css";      // Very specific
        let pattern2 = "static/*/main.css";        // Medium specific
        let pattern3 = "static/**/*.css";          // Less specific
        let pattern4 = "**";                       // Minimal specific

        let spec1 = PluginEntry::calculate_path_regex_specificity(pattern1);
        let spec2 = PluginEntry::calculate_path_regex_specificity(pattern2);
        let spec3 = PluginEntry::calculate_path_regex_specificity(pattern3);
        let spec4 = PluginEntry::calculate_path_regex_specificity(pattern4);

        // Specificity should be in descending order
        assert!(spec1 > spec2, "Exact path should be more specific than single wildcard");
        assert!(spec2 > spec3, "Single wildcard should be more specific than double wildcard");
        assert!(spec3 > spec4, "Partially specific should be better than only wildcards");
        assert!(spec4 < 0, "Only wildcards should have negative specificity");
    }
}
