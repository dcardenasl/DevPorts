//! The chain of processes that led to this command (`CLI.md`, section 3).
//!
//! It is computed here and not in the core because only this process knows it:
//! the shell that launched it, the editor or agent that launched the shell, and
//! so on. The core labels the names it recognises and shows the rest as they are.

use devports_proto::views::ChainLink;

/// How many ancestors are reported at most.
const MAX_LINKS: usize = 12;

/// This process's ancestors, nearest first, ending before `launchd`.
///
/// An empty chain when the process table cannot be read: attribution is a
/// courtesy, and nothing depends on it.
#[must_use]
pub fn chain() -> Vec<ChainLink> {
    let Some(table) = read_table() else {
        return Vec::new();
    };
    links_from(&table, std::process::id())
}

/// One row of `ps`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
}

/// The ancestors of `pid` in `table`, nearest first. Stops at `launchd`, at a
/// loop and at [`MAX_LINKS`].
#[must_use]
pub fn links_from(table: &[Row], pid: u32) -> Vec<ChainLink> {
    let mut links = Vec::new();
    let mut current = pid;
    while links.len() < MAX_LINKS {
        let Some(row) = table.iter().find(|row| row.pid == current) else {
            break;
        };
        let parent = row.ppid;
        if parent <= 1 || links.iter().any(|link: &ChainLink| link.pid == parent) {
            break;
        }
        let Some(parent_row) = table.iter().find(|candidate| candidate.pid == parent) else {
            break;
        };
        links.push(ChainLink {
            pid: parent,
            name: parent_row.name.clone(),
            label: None,
        });
        current = parent;
    }
    links
}

/// Parses the output of `ps -axo pid=,ppid=,comm=`.
#[must_use]
pub fn parse_table(text: &str) -> Vec<Row> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let own: u32 = parts.next()?.parse().ok()?;
            let parent: u32 = parts.next()?.parse().ok()?;
            let command = parts.collect::<Vec<_>>().join(" ");
            let name = command.rsplit('/').next().unwrap_or(&command).to_owned();
            (!name.is_empty()).then_some(Row {
                pid: own,
                ppid: parent,
                name,
            })
        })
        .collect()
}

// The one place the CLI reads the process table: `ps` is part of macOS and
// avoids a libproc dependency (and `unsafe`) in the client.
#[allow(clippy::disallowed_methods)]
fn read_table() -> Option<Vec<Row>> {
    let output = std::process::Command::new("ps")
        .args(["-axo", "pid=,ppid=,comm="])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| parse_table(&String::from_utf8_lossy(&output.stdout)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "\
    1     0 /sbin/launchd
  200     1 /Applications/Cursor.app/Contents/MacOS/Cursor
  300   200 /bin/zsh
  400   300 /usr/local/bin/devports
  500   400 /Applications/Google Chrome.app/Contents/MacOS/Google Chrome Helper
";

    #[test]
    fn the_table_is_parsed_with_names_that_have_spaces() {
        let rows = parse_table(TABLE);
        assert_eq!(rows.len(), 5);
        assert_eq!(
            rows.last().map(|row| row.name.as_str()),
            Some("Google Chrome Helper")
        );
    }

    #[test]
    fn the_chain_goes_up_to_launchd_and_stops() {
        let links = links_from(&parse_table(TABLE), 400);
        let names: Vec<&str> = links.iter().map(|link| link.name.as_str()).collect();
        assert_eq!(names, ["zsh", "Cursor"]);
        assert_eq!(links.first().map(|link| link.pid), Some(300));
    }

    #[test]
    fn a_process_that_is_not_in_the_table_has_no_chain() {
        assert!(links_from(&parse_table(TABLE), 999).is_empty());
    }

    #[test]
    fn a_loop_in_the_table_does_not_hang() {
        let table = parse_table("10 20 a\n20 10 b\n");
        assert!(links_from(&table, 10).len() <= 2);
    }
}
