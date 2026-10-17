//! Durations as people write them: `30m`, `2h`, `1500ms`, `1h30m`.

use std::time::Duration;

/// Why a duration could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DurationError {
    /// Nothing was written.
    #[error("empty duration")]
    Empty,
    /// A number had no unit, or the unit is not known.
    #[error("`{0}` is not a valid duration; use units like 30s, 30m, 2h or 1d")]
    Invalid(String),
    /// The total does not fit.
    #[error("the duration is too large")]
    Overflow,
}

/// Parses one or more `<number><unit>` segments, with units `ms`, `s`, `m`, `h`
/// and `d`. Segments add up, so `1h30m` is ninety minutes. Whitespace between
/// segments is allowed.
///
/// # Errors
///
/// See [`DurationError`].
pub fn parse_duration(text: &str) -> Result<Duration, DurationError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(DurationError::Empty);
    }
    let invalid = || DurationError::Invalid(text.to_owned());
    let mut total = Duration::ZERO;
    let mut rest = text;
    while !rest.is_empty() {
        rest = rest.trim_start();
        let digits_end = rest
            .find(|character: char| !character.is_ascii_digit())
            .ok_or_else(invalid)?;
        let (number, after) = rest.split_at(digits_end);
        if number.is_empty() {
            return Err(invalid());
        }
        let value: u64 = number.parse().map_err(|_| DurationError::Overflow)?;
        let unit_end = after
            .find(|character: char| !character.is_ascii_alphabetic())
            .unwrap_or(after.len());
        let (unit, remainder) = after.split_at(unit_end);
        let segment = match unit {
            "ms" => Duration::from_millis(value),
            "s" => Duration::from_secs(value),
            "m" => Duration::from_secs(value.checked_mul(60).ok_or(DurationError::Overflow)?),
            "h" => Duration::from_secs(value.checked_mul(3_600).ok_or(DurationError::Overflow)?),
            "d" => Duration::from_secs(value.checked_mul(86_400).ok_or(DurationError::Overflow)?),
            _ => return Err(invalid()),
        };
        total = total.checked_add(segment).ok_or(DurationError::Overflow)?;
        rest = remainder;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_units_are_read() {
        for (text, expected) in [
            ("1500ms", Duration::from_millis(1500)),
            ("45s", Duration::from_secs(45)),
            ("30m", Duration::from_mins(30)),
            ("2h", Duration::from_hours(2)),
            ("1d", Duration::from_hours(24)),
            ("0s", Duration::ZERO),
        ] {
            assert_eq!(parse_duration(text), Ok(expected), "{text}");
        }
    }

    #[test]
    fn segments_add_up_and_whitespace_is_tolerated() {
        assert_eq!(parse_duration("1h30m"), Ok(Duration::from_mins(90)));
        assert_eq!(
            parse_duration(" 1h 30m 10s "),
            Ok(Duration::from_secs(5410))
        );
    }

    #[test]
    fn nonsense_is_refused_with_a_message() {
        for text in ["30", "m", "30x", "1.5h", "-5m", "5 m", "h30"] {
            assert!(
                matches!(parse_duration(text), Err(DurationError::Invalid(_))),
                "{text}"
            );
        }
        assert_eq!(parse_duration(""), Err(DurationError::Empty));
        assert_eq!(parse_duration("   "), Err(DurationError::Empty));
    }

    #[test]
    fn huge_values_overflow_instead_of_wrapping() {
        assert_eq!(
            parse_duration("99999999999999999999s"),
            Err(DurationError::Overflow)
        );
        assert_eq!(
            parse_duration("18446744073709551615h"),
            Err(DurationError::Overflow)
        );
    }
}
