use regex::Regex;

pub(super) fn should_force_search(user_text: &str) -> bool {
    let jp = Regex::new(
        r"(リリースノート|変更点|変更履歴|ハイライト|新機能|何が(新しい|変わった)|教えて)",
    )
    .unwrap();
    let en = Regex::new(r"(release\s*notes?|changelog|what'?s\s*new|highlights?|patch\s*notes?)")
        .unwrap();
    let ver = Regex::new(r"\b\d+\.\d+(\.\d+)?\b").unwrap();
    jp.is_match(user_text) || en.is_match(user_text) || ver.is_match(user_text)
}
