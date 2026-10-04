//! Comparaison de versions « x.y.z » (préfixe `v` et suffixes ignorés) pour la mise à jour.

/// `(majeur, mineur, correctif)` ; `None` si aucun nombre en tête.
pub fn parse(version: &str) -> Option<(u32, u32, u32)> {
    let v = version.trim().trim_start_matches(['v', 'V']);
    let core = v
        .split(|c: char| c == '-' || c == '+' || c.is_whitespace())
        .next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u32>());
    let major = parts.next()?.ok()?;
    let minor = parts.next().unwrap_or(Ok(0)).ok()?;
    let patch = parts.next().unwrap_or(Ok(0)).ok()?;
    Some((major, minor, patch))
}

/// `candidate` est strictement plus récente que `current`. Une version illisible n'est jamais
/// « plus récente ».
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse(candidate), parse(current)) {
        (Some(c), Some(r)) => c > r,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_forms() {
        assert_eq!(parse("0.2.1"), Some((0, 2, 1)));
        assert_eq!(parse("v1.0"), Some((1, 0, 0)));
        assert_eq!(parse("2.3.4-beta.1"), Some((2, 3, 4)));
        assert_eq!(parse(" 10.0.7+build "), Some((10, 0, 7)));
        assert_eq!(parse("abc"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn newer_is_strict_and_numeric() {
        assert!(is_newer("0.2.1", "0.2.0"));
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("n/a", "0.2.0"));
        assert!(!is_newer("0.3.0", "x"));
    }
}
