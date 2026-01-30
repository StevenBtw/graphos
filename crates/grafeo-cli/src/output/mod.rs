//! Output formatting for CLI commands.

use comfy_table::{Cell, Color, ContentArrangement, Table};
use serde::Serialize;

/// Output format selection.
#[derive(Clone, Copy)]
pub enum Format {
    Table,
    Json,
}

impl From<crate::OutputFormat> for Format {
    fn from(f: crate::OutputFormat) -> Self {
        match f {
            crate::OutputFormat::Table => Format::Table,
            crate::OutputFormat::Json => Format::Json,
        }
    }
}

/// Print data as a table or JSON based on format selection.
#[allow(dead_code)]
pub fn print_output<T: Serialize>(data: &T, format: Format, quiet: bool) -> anyhow::Result<()> {
    if quiet {
        return Ok(());
    }

    match format {
        Format::Json => {
            println!("{}", serde_json::to_string_pretty(data)?);
        }
        Format::Table => {
            // For table format, we'll let individual commands handle formatting
            // since each has different structure
            println!("{}", serde_json::to_string_pretty(data)?);
        }
    }
    Ok(())
}

/// Create a styled table with consistent formatting.
pub fn create_table() -> Table {
    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.load_preset(comfy_table::presets::UTF8_FULL_CONDENSED);
    table
}

/// Add a header row to a table.
pub fn add_header(table: &mut Table, headers: &[&str]) {
    table.set_header(
        headers
            .iter()
            .map(|h| Cell::new(h).fg(Color::Cyan))
            .collect::<Vec<_>>(),
    );
}

/// Print a key-value table (for info displays).
pub fn print_key_value_table(items: &[(&str, String)], format: Format, quiet: bool) {
    if quiet {
        return;
    }

    match format {
        Format::Json => {
            let map: std::collections::HashMap<&str, &str> =
                items.iter().map(|(k, v)| (*k, v.as_str())).collect();
            println!("{}", serde_json::to_string_pretty(&map).unwrap());
        }
        Format::Table => {
            let mut table = create_table();
            add_header(&mut table, &["Property", "Value"]);
            for (key, value) in items {
                table.add_row(vec![Cell::new(key).fg(Color::Green), Cell::new(value)]);
            }
            println!("{table}");
        }
    }
}

/// Print a status message (respects quiet mode).
pub fn status(msg: &str, quiet: bool) {
    if !quiet {
        println!("{msg}");
    }
}

/// Print a success message.
pub fn success(msg: &str, quiet: bool) {
    if !quiet {
        println!("✓ {msg}");
    }
}

/// Print an error message.
#[allow(dead_code)]
pub fn error(msg: &str) {
    eprintln!("✗ {msg}");
}
