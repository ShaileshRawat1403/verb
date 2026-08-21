//! Minimal, allocation-light JSON field readers.
//!
//! Verb takes no dependencies for this: every use is reading two or three known scalar fields out of
//! a line it does not otherwise interpret -- an agent's session metadata, or one of Verb's own
//! structural events. Anything that needed real parsing would be a sign Verb was reading something
//! it should not be.

pub(crate) fn json_string(input: &str, key: &str) -> Option<String> {
    json_strings(input, key).into_iter().next()
}

pub(crate) fn json_strings(input: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\":");
    let mut values = Vec::new();
    let mut rest = input;
    while let Some(index) = rest.find(&needle) {
        let after = &rest[index + needle.len()..];
        let after = after.trim_start();
        rest = after;
        let Some(body) = after.strip_prefix('"') else {
            continue;
        };
        let mut value = String::new();
        let mut characters = body.chars();
        let mut escaped = false;
        for character in characters.by_ref() {
            if escaped {
                value.push(character);
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                values.push(std::mem::take(&mut value));
                break;
            } else {
                value.push(character);
            }
        }
    }
    values
}

pub(crate) fn json_number(input: &str, key: &str) -> Option<u128> {
    let needle = format!("\"{key}\":");
    let index = input.find(&needle)?;
    let after = input[index + needle.len()..].trim_start();
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

