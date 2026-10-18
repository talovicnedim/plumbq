//! The one rule for "does this variable, header or field name hold a credential?". Masking
//! errs on the side of too much: a name is secret when it contains a credential word, unless it
//! ends in a word that says it is not the credential itself (`tokenUrl`, `cookieDomain`).

/// Parts of a name (separators and case ignored) that always make it secret.
const STRONG_HINTS: [&str; 9] = [
    "secret", "pass", "pwd", "cred", "dsn", "private", "apikey", "csrf", "xsrf",
];

/// Parts that make a name secret unless it ends in one of [`BENIGN_LAST_WORDS`]. `pat` only
/// counts as a whole word.
const WEAK_HINTS: [&str; 7] = ["token", "key", "auth", "cookie", "session", "cert", "pat"];

/// Last words of names that describe a credential, not hold it (`tokenUrl`, `cookieDomain`).
const BENIGN_LAST_WORDS: [&str; 14] = [
    "url", "uri", "endpoint", "domain", "path", "timeout", "ttl", "expiry", "expires", "maxage",
    "samesite", "type", "name", "length",
];

/// Names that only look like credentials (`author` has `auth`, `keyboard` has `key`).
const BENIGN_NAMES: [&str; 3] = ["author", "authors", "keyboard"];

pub fn looks_secret(name: &str) -> bool {
    let squashed: String = name
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    if BENIGN_NAMES.contains(&squashed.as_str()) {
        return false;
    }
    if STRONG_HINTS.iter().any(|hint| squashed.contains(hint)) {
        return true;
    }
    let words = words(name);
    let weak = WEAK_HINTS.iter().any(|hint| match *hint {
        "pat" => words.iter().any(|word| word == "pat"),
        _ => squashed.contains(hint),
    });
    let benign = words
        .last()
        .is_some_and(|last| BENIGN_LAST_WORDS.contains(&last.as_str()));
    weak && !benign
}

/// Lower-case words split at anything that isn't a letter or digit and at camelCase borders,
/// without the numbers a name ends in (`token2`).
fn words(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut words = vec![String::new()];
    for (at, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            words.push(String::new());
            continue;
        }
        let previous = at.checked_sub(1).map(|before| chars[before]);
        let next = chars.get(at + 1);
        let border = c.is_uppercase()
            && previous.is_some_and(|p| {
                p.is_lowercase()
                    || p.is_ascii_digit()
                    || (p.is_uppercase() && next.is_some_and(|n| n.is_lowercase()))
            });
        if border {
            words.push(String::new());
        }
        if let Some(word) = words.last_mut() {
            word.extend(c.to_lowercase());
        }
    }
    words
        .into_iter()
        .map(|word| {
            word.trim_end_matches(|c: char| c.is_ascii_digit())
                .to_string()
        })
        .filter(|word| !word.is_empty())
        .collect()
}

#[cfg(test)]
#[path = "secret_names_tests.rs"]
mod tests;
