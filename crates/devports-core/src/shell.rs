//! POSIX shell-word formatting shared by registration and adoption.

/// Quotes one argument for `sh`, so it reaches the command exactly as given.
pub(crate) fn shell_quote(argument: &str) -> String {
    let safe = !argument.is_empty()
        && argument.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '-' | '_' | '.' | '/' | ':' | '=' | '@' | '%' | '+' | ',')
        });
    if safe {
        argument.to_owned()
    } else {
        format!("'{}'", argument.replace('\'', "'\\''"))
    }
}
