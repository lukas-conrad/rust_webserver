use crate::control_system::models::Message;
use serde::{Deserialize, Serialize};

pub fn format_table(headers: &[String], rows: Vec<Vec<String>>) -> String {
    if rows.is_empty() {
        return "Empty table".to_string();
    }

    let mut output = String::new();

    // Calculate column widths - start with header widths
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();

    // Update widths based on row data
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() && cell.len() > widths[i] {
                widths[i] = cell.len();
            }
        }
    }

    // Create separator line
    let separator = widths.iter()
        .map(|w| "-".repeat(w + 2))
        .collect::<Vec<_>>()
        .join("+");
    let separator = format!("+{}+\n", separator);

    // Add top border
    output.push_str(&separator);

    // Add header row
    output.push('|');
    for (i, header) in headers.iter().enumerate() {
        output.push_str(&format!(" {:width$} |", header, width = widths[i]));
    }
    output.push('\n');
    output.push_str(&separator);

    // Add data rows
    for row in rows {
        output.push('|');
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                output.push_str(&format!(" {:width$} |", cell, width = widths[i]));
            }
        }
        output.push('\n');
    }

    // Add bottom border
    output.push_str(&separator);

    output
}

macro_rules! table {
    (
        $([$import:item])*
        $name:ident {
            $($field:ident : $ty:ty),* $(,)?
        }
    ) => {
        pub mod $name {
            use std::fmt::Display;
            use crate::control_system::models::Message;
            use serde::{Deserialize, Serialize};
            $($import)*

            #[derive(Serialize, Deserialize)]
            pub struct Table {
                pub header: Vec<String>,
                pub columns: Vec<Column>
            }

            impl Table {
                pub fn new(columns: Vec<Column>) -> Self {
                    Self {
                        header: vec![$(stringify!($field).to_string()),*],
                        columns
                    }
                }
            }

            #[derive(Serialize, Deserialize)]
            pub struct Column {
                $(pub $field: $ty), *
            }

            const _: () = {
                fn assert_serialize<T: Serialize>() {}
                fn assert_display<T: Display>() {}
                fn assert_deserialize<T: for<'de> Deserialize<'de>>() {}

                fn check_traits() {
                    $(
                        assert_serialize::<$ty>();
                        assert_deserialize::<$ty>();
                        assert_display::<$ty>();
                    )*
                }
            };

            impl Message for Table {
                fn to_string(&self) -> String {
                    let columns: Vec<Vec<String>> = self.columns.iter().map(|column| {
                        vec![
                            $(column.$field.to_string()),*
                        ]
                    }).collect();

                    crate::control_system::commands::models::format_table(&self.header, columns)
                }

                fn to_json(&self) -> String {
                    serde_json::to_string(self).unwrap_or("{}".to_string())
                }
            }
        }
    };
}

#[derive(Serialize, Deserialize)]
pub struct TextMessage {
    pub text: String,
}

impl TextMessage {
    pub fn new(string: String) -> Self {
        Self { text: string }
    }
}

impl Message for TextMessage {
    fn to_string(&self) -> String {
        self.text.clone()
    }

    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or("{}".to_string())
    }
}

table! {
    [use crate::plugin::interfaces::State;]
    plugin_table {
        name: String,
        state: State,
        protocols: String,
        startup_command: String,
        max_request_timeout: String,
        request_methods: String,
        hosts: String,
        paths: String
    }
}