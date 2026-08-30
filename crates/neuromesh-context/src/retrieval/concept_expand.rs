//! Query-side concept expansion: NL terms → likely code identifier variants.

/// Generate camelCase, PascalCase, and snake_case variants from a phrase or term.
pub fn identifier_variants(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if trimmed.contains('_')
        || (trimmed
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase())
            && trimmed.chars().any(|c| c.is_ascii_lowercase()))
    {
        return vec![trimmed.to_string()];
    }

    let words: Vec<String> = trimmed
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    if words.len() == 1 {
        let w = &words[0];
        return vec![w.clone(), to_pascal(w), to_camel(w)];
    }

    let snake = words.join("_");
    let camel = to_camel_from_words(&words);
    let pascal = to_pascal_from_words(&words);
    vec![camel, pascal, snake]
}

fn to_pascal(word: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for c in word.chars() {
        if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn to_camel(word: &str) -> String {
    let p = to_pascal(word);
    if p.is_empty() {
        return p;
    }
    let mut chars = p.chars();
    let first = chars.next().unwrap().to_ascii_lowercase();
    format!("{first}{}", chars.as_str())
}

fn to_pascal_from_words(words: &[String]) -> String {
    words.iter().map(|w| to_pascal(w)).collect()
}

fn to_camel_from_words(words: &[String]) -> String {
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        if i == 0 {
            out.push_str(w);
        } else {
            out.push_str(&to_pascal(w));
        }
    }
    out
}

/// Expand matched English concept tokens into concrete code identifier guesses.
pub fn expand_concept_to_code_seeds(concept: &str) -> Vec<String> {
    match concept {
        "auth" | "jwt" | "token" => vec![
            "validateToken".into(),
            "verifyJwt".into(),
            "JwtPayload".into(),
            "authMiddleware".into(),
            "token_expires".into(),
            "authenticate".into(),
        ],
        "middleware" => vec!["middleware".into(), "app.use".into(), "next".into()],
        "routing" => vec!["Router".into(), "route".into(), "app".into()],
        "render" => vec![
            "res.render".into(),
            "render".into(),
            "view".into(),
            "engine".into(),
        ],
        "session" => vec!["session".into(), "cookie".into(), "cookie-session".into()],
        "query" => vec!["req.query".into(), "query".into(), "parseurl".into()],
        "database" => vec!["model".into(), "repository".into(), "database".into()],
        "config" => vec!["config".into(), "settings".into(), "env".into()],
        "error" => vec!["error".into(), "exception".into(), "handler".into()],
        "content_type" => vec![
            "addContentTypeParser".into(),
            "contentTypeParser".into(),
            "content-type-parser".into(),
        ],
        "plugin" => vec![
            "register".into(),
            "plugin-utils".into(),
            "encapsulate".into(),
        ],
        "validation" => vec![
            "validation".into(),
            "schemas".into(),
            "schemaController".into(),
        ],
        "errors" => vec![
            "error-handler".into(),
            "error-serializer".into(),
            "setErrorHandler".into(),
        ],
        other => identifier_variants(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jwt_concept_yields_validate_token_variants() {
        let seeds = expand_concept_to_code_seeds("jwt");
        assert!(seeds.iter().any(|s| s.contains("Jwt") || s.contains("jwt")));
        assert!(seeds.iter().any(|s| s == "validateToken"));
    }

    #[test]
    fn phrase_to_camel_and_snake() {
        let v = identifier_variants("validate token");
        assert!(v.iter().any(|s| s == "validateToken"));
        assert!(v.iter().any(|s| s == "validate_token"));
    }
}
