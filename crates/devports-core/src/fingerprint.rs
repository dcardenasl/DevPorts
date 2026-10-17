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
    /// When the run starts at the very beginning, the first token is the program
    /// and is compared by its [`program_family`], so `python3`, `python3.14` and
    /// the `Python` that Homebrew re-executes itself as all match each other.
    /// Everywhere else tokens must be equal.
    ///
    /// An empty `needle` is contained in nothing: a service with an empty
    /// `match` has no fingerprint to recognise.
    #[must_use]
    pub fn contains(&self, needle: &Self) -> bool {
        let Some((first, rest)) = needle.tokens.split_first() else {
            return false;
        };
        self.tokens
            .windows(needle.tokens.len())
            .enumerate()
            .any(|(start, window)| {
                let Some((head, tail)) = window.split_first() else {
                    return false;
                };
                let same_head = if start == 0 {
                    head == first || program_family(head) == program_family(first)
                } else {
                    head == first
                };
                same_head && tail == rest
            })
    }

    /// The tokens joined with single spaces, as shown to people.
    #[must_use]
    pub fn render(&self) -> String {
        self.tokens.join(" ")
    }
}

/// The name of a program with the parts that vary between installs removed: lower
/// case, and a trailing version (`python3.14` -> `python`, `php8.2` -> `php`).
///
/// A version is stripped only when what remains is at least two characters and
/// ends in a letter, so `h264` and `x1` are left alone. The result never ends in
/// a digit, which makes the function idempotent.
#[must_use]
pub fn program_family(name: &str) -> String {
    let lower = name.to_lowercase();
    let without_version =
        lower.trim_end_matches(|character: char| character.is_ascii_digit() || character == '.');
    let versioned = without_version.len() < lower.len();
    let ends_in_letter = without_version
        .chars()
        .next_back()
        .is_some_and(char::is_alphabetic);
    if versioned && ends_in_letter && without_version.chars().count() >= 2 {
        without_version.to_owned()
    } else {
        lower
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
    // covers: U-01
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

    // covers: U-01
    #[test]
    fn the_program_is_reduced_to_its_base_name_even_when_relative() {
        assert_eq!(normalized("./node_modules/.bin/vite dev"), "vite dev");
        assert_eq!(normalized("~/bin/serve"), "serve");
    }

    // covers: U-01
    #[test]
    fn only_ports_in_the_replaceable_range_change() {
        assert_eq!(normalized("srv 80 443 1023"), "srv 80 443 1023");
        assert_eq!(normalized("srv 1024 65535 65536"), "srv N N 65536");
        assert_eq!(normalized("srv --port=80"), "srv --port=80");
        assert_eq!(normalized("srv 0.0.0.0:8080"), "srv 0.0.0.0:N");
        assert_eq!(normalized("srv [::1]:3000"), "srv [::1]:N");
    }

    // covers: U-01
    #[test]
    fn a_port_inside_a_larger_value_is_found_after_the_last_separator() {
        assert_eq!(
            normalized("srv --host=localhost:3000"),
            "srv --host=localhost:N"
        );
    }

    // covers: U-01
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

    // covers: U-01
    #[test]
    fn the_program_is_compared_by_family() {
        // Homebrew's framework Python re-executes itself as `Python`.
        let homebrew = Fingerprint::from_argv(&[
            "/opt/homebrew/Cellar/python@3.14/3.14.2/Frameworks/Python.framework/Versions/3.14/Resources/Python.app/Contents/MacOS/Python",
            "-m",
            "http.server",
            "18083",
        ]);
        for needle in [
            "python3 -m http.server N",
            "python -m http.server N",
            "Python -m http.server N",
            "python3.14 -m http.server N",
            "-m http.server N",
        ] {
            assert!(
                homebrew.contains(&Fingerprint::from_match_text(needle)),
                "{needle}"
            );
        }
        assert!(!homebrew.contains(&Fingerprint::from_match_text("node -m http.server N")));

        let php = Fingerprint::from_argv(&["php8.2", "spark", "serve"]);
        assert!(php.contains(&Fingerprint::from_match_text("php spark serve")));
        assert!(php.contains(&Fingerprint::from_match_text("PHP8.2 spark serve")));
    }

    #[test]
    fn family_folding_applies_only_to_the_program_position() {
        let process = Fingerprint::from_argv(&["node", "server2", "Serve3"]);
        // Mid-command tokens are compared exactly: `server` is not `server2`.
        assert!(!process.contains(&Fingerprint::from_match_text("server")));
        assert!(!process.contains(&Fingerprint::from_match_text("serve3")));
        assert!(process.contains(&Fingerprint::from_match_text("server2 Serve3")));
        // A needle that starts mid-command must not be folded either.
        assert!(!process.contains(&Fingerprint::from_match_text("Server2")));
    }

    #[test]
    fn program_family_strips_only_trailing_versions() {
        for (name, family) in [
            ("python3", "python"),
            ("python3.14", "python"),
            ("Python", "python"),
            ("php8.2", "php"),
            ("node", "node"),
            ("h264", "h264"),
            ("x1", "x1"),
            ("vite", "vite"),
            ("a1b2", "a1b"),
            ("node-18", "node-18"),
            ("python3.", "python"),
            ("foo.2", "foo"),
        ] {
            assert_eq!(program_family(name), family, "for {name}");
        }
    }

    #[test]
    fn an_empty_needle_matches_nothing() {
        let process = Fingerprint::from_argv(&["php"]);
        assert!(!process.contains(&Fingerprint::default()));
        assert!(!Fingerprint::default().contains(&Fingerprint::default()));
    }

    proptest! {
        /// PT-01: normalising twice gives the same tokens as normalising once.
        // covers: PT-01
        #[test]
        fn normalization_is_idempotent(argv in prop::collection::vec("\\PC{0,14}", 0..8)) {
            let once = Fingerprint::from_argv(&argv);
            let twice = Fingerprint::from_argv(once.tokens());
            prop_assert_eq!(once, twice);
        }

        /// The text form is stable too, for tokens that survive a whitespace split.
        // covers: PT-01
        #[test]
        fn rendered_text_is_a_fixed_point(argv in prop::collection::vec("\\S{1,14}", 0..8)) {
            let once = Fingerprint::from_argv(&argv).render();
            prop_assert_eq!(Fingerprint::from_match_text(&once).render(), once);
        }

        /// Folding a program name twice changes nothing.
        // covers: PT-01
        #[test]
        fn program_family_is_idempotent(name in "\\PC{0,14}") {
            let once = program_family(&name);
            prop_assert_eq!(program_family(&once), once);
        }

        /// Any in-range port is erased, whichever way it is written.
        // covers: PT-02
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
