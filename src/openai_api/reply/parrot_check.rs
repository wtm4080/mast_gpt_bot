fn normalize_for_parrot(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect::<String>()
}

pub(super) fn is_parrot_reply(user_text: &str, reply_text: &str) -> bool {
    let u = normalize_for_parrot(user_text);
    let r = normalize_for_parrot(reply_text);
    !u.is_empty() && !r.is_empty() && u == r
}
