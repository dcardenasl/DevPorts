//! Command-line fingerprint (`MANIFIESTO.md`, section 3).
//!
//! A service is recognised by what it runs, never by its port, because
//! frameworks hop from 3000 to 3001 when a port is taken (ADR-003). The
//! fingerprint is the command line with everything that varies between runs
//! removed: directories, and port numbers.
//!
//! The same normalisation is applied to an observed process and to the `match`
//! value of a manifest, so the two can be compared token by token.

/// Lowest port number replaced by `N`. Below it the number is more likely a
/// protocol constant (`80`, `443`) than a port chosen for this run.
const PORT_MIN: u32 = 1024;
/// Highest TCP port.
const PORT_MAX: u32 = 65_535;
/// What a replaced port number becomes.
const PORT_PLACEHOLDER: &str = "N";

/// A normalised command line, as a list of tokens.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Fingerprint {
    tokens: Vec<String>,
}

impl Fingerprint {
    /// Fingerprint of an observed process's `argv`.
    #[must_use]
    pub fn from_argv<S: AsRef<str>>(argv: &[S]) -> Self {
        Self {
            tokens: argv
                .iter()
                .enumerate()
                .map(|(index, token)| normalize_token(token.as_ref(), index == 0))
                .collect(),
        }
    }

    /// Fingerprint of a manifest's `match` text, split on whitespace.
    #[must_use]
    pub fn from_match_text(text: &str) -> Self {
        Self::from_argv(&text.split_whitespace().collect::<Vec<_>>())
    }

    /// The normalised tokens.
    #[must_use]
    pub fn tokens(&self) -> &[String] {
        &self.tokens
    }

    /// Number of tokens.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Whether there are no tokens at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Whether `needle` appears in `self` as a run of consecutive tokens.
    ///
    /// An empty `needle` is contained in nothing: a service with an empty
    /// `match` has no fingerprint to recognise.
    #[must_use]
    pub fn contains(&self, needle: &Self) -> bool {
        !needle.tokens.is_empty()
            && self
                .tokens
                .windows(needle.tokens.len())
                .any(|window| window == needle.tokens.as_slice())
    }

    /// The tokens joined with single spaces, as shown to people.
    #[must_use]
    pub fn render(&self) -> String {
        self.tokens.join(" ")
    }
}

/// Normalises one token. `is_program` is true for `argv[0]`, which is reduced to
/// its base name even when it is a relative path.
fn normalize_token(token: &str, is_program: bool) -> String {
    let token = if is_program || token.starts_with('/') || token.starts_with('~') {
        base_name(token)
    } else {
        token
    };
    if is_port(token) {
        return PORT_PLACEHOLDER.to_owned();
    }
    for separator in ['=', ':'] {
        if let Some((head, tail)) = token.rsplit_once(separator)
            && is_port(tail)
        {
            return format!("{head}{separator}{PORT_PLACEHOLDER}");
        }
    }
    token.to_owned()
}

/// The last non-empty path component, or the whole token if there is none.
fn base_name(token: &str) -> &str {
    token
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(token)
}

/// Whether `text` is a decimal number in the replaceable port range.
fn is_port(text: &str) -> bool {
    // Five digits cover the whole range; refusing longer text also avoids overflow.
    !text.is_empty()
        && text.len() <= 5
        && text.bytes().all(|byte| byte.is_ascii_digit())
        && text
            .parse::<u32>()
            .is_ok_and(|number| (PORT_MIN..=PORT_MAX).contains(&number))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn normalized(line: &str) -> String {
        Fingerprint::from_match_text(line).render()
    }

    /// U-01: every row of the table in MANIFIESTO section 3.
    #[test]
    fn the_documented_examples_normalize_as_documented() {
        let cases = [
            (
                "/usr/bin/php spark serve --port 8080",
                "php spark serve --port N",
            ),
            (
                "node /Users/x/app/node_modules/.bin/vite --port=5173",
                "node vite --port=N",
            ),
            ("python3 -m http.server 8001", "python3 -m http.server N"),
            (
                "php -S localhost:8000 -t public",
                "php -S localhost:N -t public",
            ),
        ];
        for (line, expected) in cases {
            assert_eq!(normalized(line), expected, "for `{line}`");
        }
    }

    #[test]
    fn the_program_is_reduced_to_its_base_name_even_when_relative() {
        assert_eq!(normalized("./node_modules/.bin/vite dev"), "vite dev");
        assert_eq!(normalized("~/bin/serve"), "serve");
    }

    #[test]
    fn only_ports_in_the_replaceable_range_change() {
        assert_eq!(normalized("srv 80 443 1023"), "srv 80 443 1023");
        assert_eq!(normalized("srv 1024 65535 65536"), "srv N N 65536");
        assert_eq!(normalized("srv --port=80"), "srv --port=80");
        assert_eq!(normalized("srv 0.0.0.0:8080"), "srv 0.0.0.0:N");
        assert_eq!(normalized("srv [::1]:3000"), "srv [::1]:N");
    }

    #[test]
    fn a_port_inside_a_larger_value_is_found_after_the_last_separator() {
        assert_eq!(
            normalized("srv --host=localhost:3000"),
            "srv --host=localhost:N"
        );
    }

    #[test]
    fn tokens_that_only_look_like_numbers_are_left_alone() {
        assert_eq!(
            normalized("srv 12a45 -1500 +8080 1e4"),
            "srv 12a45 -1500 +8080 1e4"
        );
    }

    #[test]
    fn a_numeric_base_name_is_replaced_in_the_same_pass() {
        // `/opt/8080` becomes `8080`; leaving that for a second pass would break
        // idempotence.
        assert_eq!(normalized("srv /opt/8080"), "srv N");
    }

    #[test]
    fn contains_needs_consecutive_tokens() {
        let process = Fingerprint::from_argv(&["php", "spark", "serve", "--port", "8080"]);
        assert!(process.contains(&Fingerprint::from_match_text("php spark serve")));
        assert!(process.contains(&Fingerprint::from_match_text("serve --port N")));
        assert!(!process.contains(&Fingerprint::from_match_text("php serve")));
        assert!(!process.contains(&Fingerprint::from_match_text("serve spark")));
        assert!(!process.contains(&Fingerprint::from_match_text(
            "php spark serve --port N extra"
        )));
    }

    #[test]
    fn an_empty_needle_matches_nothing() {
        let process = Fingerprint::from_argv(&["php"]);
        assert!(!process.contains(&Fingerprint::default()));
        assert!(!Fingerprint::default().contains(&Fingerprint::default()));
    }

    proptest! {
        /// PT-01: normalising twice gives the same tokens as normalising once.
        #[test]
        fn normalization_is_idempotent(argv in prop::collection::vec("\\PC{0,14}", 0..8)) {
            let once = Fingerprint::from_argv(&argv);
            let twice = Fingerprint::from_argv(once.tokens());
            prop_assert_eq!(once, twice);
        }

        /// The text form is stable too, for tokens that survive a whitespace split.
        #[test]
        fn rendered_text_is_a_fixed_point(argv in prop::collection::vec("\\S{1,14}", 0..8)) {
            let once = Fingerprint::from_argv(&argv).render();
            prop_assert_eq!(Fingerprint::from_match_text(&once).render(), once);
        }

        /// Any in-range port is erased, whichever way it is written.
        #[test]
        fn a_port_never_changes_the_fingerprint(a in 1024_u32..=65_535, b in 1024_u32..=65_535) {
            for template in ["srv --port {}", "srv --port={}", "srv localhost:{}", "srv {}"] {
                let first = template.replace("{}", &a.to_string());
                let second = template.replace("{}", &b.to_string());
                prop_assert_eq!(normalized(&first), normalized(&second));
            }
        }
    }
}
