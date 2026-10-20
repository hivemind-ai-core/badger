//! Masking credentials in settings the tools print (`badger inspect
//! config`, `badger doctor`), whose types are not known there: a value is
//! redacted when any part of its key names a credential, and a URL's
//! password or a credential query parameter is masked in any other value.

const REDACTED: &str = "[redacted]";

/// Words of a key naming a credential (`db_password`, `secrets.github`,
/// `apiKey`, `BADGER_AUTH_TOKEN`).
const WORDS: [&str; 21] = [
    "password",
    "passwords",
    "passwd",
    "pass",
    "pwd",
    "passphrase",
    "secret",
    "secrets",
    "token",
    "tokens",
    "auth",
    "authorization",
    "credential",
    "credentials",
    "creds",
    "dsn",
    "cookie",
    "signature",
    "apikey",
    "privatekey",
    "pin",
];

/// Adjacent words that together name a credential (`api_key`,
/// `private_key`, `access_key_id`, `secret_key`).
const PAIRS: [(&str, &str); 5] =
    [("api", "key"), ("private", "key"), ("access", "key"), ("secret", "key"), ("signing", "key")];

/// The words of a key: split at `.`, `_`, `-`, and lower-to-upper case
/// changes, lowercased.
fn words(key: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut prev_lower = false;
    for c in key.chars() {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
            continue;
        }
        if c.is_uppercase() && prev_lower && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        prev_lower = c.is_lowercase() || c.is_ascii_digit();
        cur.extend(c.to_lowercase());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Does a key (any of its segments) name a credential? A count or size of
/// them (`tokens_count`, `max_tokens`) does not.
pub fn sensitive_key(key: &str) -> bool {
    let ws = words(key);
    let counted = ws.iter().any(|w| {
        matches!(w.as_str(), "count" | "max" | "min" | "limit" | "len" | "length" | "size" | "ttl" | "expiry")
    });
    let named = ws.iter().any(|w| WORDS.contains(&w.as_str()))
        || ws.windows(2).any(|p| PAIRS.contains(&(p[0].as_str(), p[1].as_str())));
    named && !counted
}

/// A setting as shown: redacted when its key names a credential, with
/// URL passwords and credential query parameters masked otherwise. A
/// reference to the environment (`${NAME}`) is shown as written.
pub fn setting(key: &str, value: &str) -> String {
    if value.starts_with("${") && value.ends_with('}') {
        return value.to_string();
    }
    let scalar = value.parse::<f64>().is_ok() || value == "true" || value == "false";
    if sensitive_key(key) && !(scalar && value.len() < 8 && !is_pin(key)) {
        return REDACTED.to_string();
    }
    urls(value)
}

/// A PIN is a credential even when it is a number.
fn is_pin(key: &str) -> bool {
    words(key).iter().any(|w| w == "pin" || w.starts_with("pass") || w == "pwd")
}

/// Mask every URL's password (`scheme://user:password@host`) and
/// credential query parameters (`?token=...`) in a value, which may hold
/// several (a list).
pub fn urls(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(i) = rest.find("://") {
        out.push_str(&rest[..i + 3]);
        rest = &rest[i + 3..];
        // the URL runs to whitespace, a quote, or a list separator
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ']'))
            .unwrap_or(rest.len());
        out.push_str(&mask_url(&rest[..end]));
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// One URL after its `://`.
fn mask_url(url: &str) -> String {
    let authority_end = url.find(['/', '?', '#']).unwrap_or(url.len());
    // the authority's userinfo ends at its last `@` (a password may hold one)
    let (authority, path) = url.split_at(authority_end);
    let authority = match authority.rfind('@') {
        Some(at) => {
            let info = &authority[..at];
            match info.find(':') {
                Some(colon) => format!("{}:{REDACTED}{}", &info[..colon], &authority[at..]),
                None => authority.to_string(),
            }
        }
        None => authority.to_string(),
    };
    let path = match path.find('?') {
        Some(q) => {
            let (before, query) = path.split_at(q + 1);
            let (query, fragment) = match query.find('#') {
                Some(h) => query.split_at(h),
                None => (query, ""),
            };
            let masked: Vec<String> = query
                .split('&')
                .map(|kv| match kv.split_once('=') {
                    Some((k, _)) if sensitive_key(k) => format!("{k}={REDACTED}"),
                    _ => kv.to_string(),
                })
                .collect();
            format!("{before}{}{fragment}", masked.join("&"))
        }
        None => path.to_string(),
    };
    format!("{authority}{path}")
}

#[cfg(test)]
mod tests {
    use super::{sensitive_key, setting};

    #[test]
    fn keys_naming_credentials() {
        for k in [
            "password",
            "db_password",
            "PASSWORD",
            "apiKey",
            "api_key",
            "secret_token",
            "auth",
            "passphrase",
            "private_key",
            "access_key_id",
            "secrets.github",
            "passwords.value",
            "BADGER_AUTH_TOKEN",
            "dsn",
        ] {
            assert!(sensitive_key(k), "{k}");
        }
        for k in ["note", "author", "tokens_count", "max_tokens", "passenger", "keyboard", "url"] {
            assert!(!sensitive_key(k), "{k}");
        }
    }

    #[test]
    fn values() {
        assert_eq!(setting("password", "pw"), "[redacted]");
        assert_eq!(setting("password", "${DB_PASSWORD}"), "${DB_PASSWORD}");
        assert_eq!(setting("tokens_count", "5"), "5");
        assert_eq!(setting("pin", "1234"), "[redacted]");
        assert_eq!(setting("atpw", "postgres://u:p@ss@host/db"), "postgres://u:[redacted]@host/db");
        assert_eq!(setting("redis", "redis://:only@localhost:6379"), "redis://:[redacted]@localhost:6379");
        assert_eq!(
            setting("list", r#"["postgres://a:first@c/d", "postgres://e:second@g/h"]"#),
            r#"["postgres://a:[redacted]@c/d", "postgres://e:[redacted]@g/h"]"#
        );
        assert_eq!(
            setting("plain", "https://example.com/?token=abc&page=2&password=xyz#top"),
            "https://example.com/?token=[redacted]&page=2&password=[redacted]#top"
        );
        assert_eq!(setting("repo", "ssh://git@github.com/x"), "ssh://git@github.com/x");
        assert_eq!(setting("note", "hello"), "hello");
    }
}
