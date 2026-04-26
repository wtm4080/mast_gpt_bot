use once_cell::sync::Lazy;
use regex::Regex;

static JP_RELEASE_QUERY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(リリースノート|変更点|変更履歴|ハイライト|新機能|何が(新しい|変わった)|教えて)")
        .unwrap()
});
static EN_RELEASE_QUERY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(release\s*notes?|changelog|what'?s\s*new|highlights?|patch\s*notes?)").unwrap()
});
static VERSION_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b\d+\.\d+(\.\d+)?\b").unwrap());

pub(super) fn should_force_search(user_text: &str) -> bool {
    JP_RELEASE_QUERY_RE.is_match(user_text)
        || EN_RELEASE_QUERY_RE.is_match(user_text)
        || VERSION_RE.is_match(user_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_japanese_release_queries() {
        assert!(should_force_search("Rust 1.91.1 のリリースノート教えて"));
        assert!(should_force_search("新機能は何が変わった？"));
    }

    #[test]
    fn detects_english_release_queries() {
        assert!(should_force_search("what's new in Rust"));
        assert!(should_force_search("show me the changelog"));
    }

    #[test]
    fn detects_version_numbers() {
        assert!(should_force_search("1.91"));
        assert!(should_force_search("1.91.1"));
    }

    #[test]
    fn does_not_force_search_for_plain_chat() {
        assert!(!should_force_search("今日のお昼なに食べよう"));
    }
}
