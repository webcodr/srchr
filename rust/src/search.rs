/// Smart-case: case-sensitive only when the query contains an uppercase char.
pub fn is_case_sensitive(query: &str) -> bool {
    query.chars().any(|c| c.is_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercase_query_is_case_insensitive() {
        assert!(!is_case_sensitive("todo"));
    }

    #[test]
    fn uppercase_char_makes_it_case_sensitive() {
        assert!(is_case_sensitive("Todo"));
    }
}
