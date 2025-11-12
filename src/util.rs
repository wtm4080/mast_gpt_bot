//! strip_html とか小物ユーティリティ

use regex::Regex;

/// Mastodon status.content (HTML) をざっくりプレーンテキストに
pub fn strip_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for c in input.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }

    out.replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .trim()
        .to_string()
}

/// Markdownリンク・生URLをドメインのプレーン表記に正規化する
/// - `[label](https://sub.example.com/a?b)` → `(sub.example.com)`
/// - `https://example.com/x` → `(example.com)`
/// - ラベルにURLが重複してもドメインだけ残す
pub fn normalize_links_to_domains(input: &str) -> String {
    // 1) Markdownリンク → (domain)
    let re_md = Regex::new(r"\[([^]]+)]\((https?://[^\s)]+)\)").unwrap();
    let dom = Regex::new(r"^https?://([^/\s?]+)").unwrap();
    let s = re_md
        .replace_all(input, |caps: &regex::Captures| {
            let url = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let domain = dom
                .captures(url)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str())
                .unwrap_or("source");
            format!("({})", domain)
        })
        .into_owned();

    // 2) 生URL → (domain)
    let re_url = Regex::new(r"https?://[^\s)]+").unwrap();
    let s = re_url
        .replace_all(&s, |caps: &regex::Captures| {
            let url = caps.get(0).map(|m| m.as_str()).unwrap_or("");
            let domain = dom
                .captures(url)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str())
                .unwrap_or("source");
            format!("({})", domain)
        })
        .into_owned();

    // 3) 余計な二重括弧や空白を軽く整理
    let s = s.replace("（", "(").replace("）", ")"); // 全角→半角
    let s = Regex::new(r"\s+").unwrap().replace_all(&s, " "); // 複数空白→1つ
    s.trim().to_string()
}

/// Mastodon上限以内におさめる（リンクはプレーン化→収める）
pub fn fit_for_mastodon_plain(input: &str, limit: usize) -> String {
    // まずリンクやURLをプレーンに正規化
    let mut s = normalize_links_to_domains(input);
    s = s.replace("　", " "); // 全角空白の除去
    s = s.trim().to_string();

    if s.chars().count() <= limit {
        return s;
    }

    // 箇条書き優先で詰める
    let lines: Vec<&str> = s.lines().collect();
    let mut acc = String::new();
    for line in lines {
        let tentative = if acc.is_empty() {
            line.to_string()
        } else {
            format!("{acc}\n{line}")
        };
        if tentative.chars().count() <= limit {
            acc = tentative;
        } else {
            break;
        }
    }
    if !acc.is_empty() {
        return acc;
    }

    // それでもダメなら末尾省略
    let mut out = String::new();
    for ch in s.chars() {
        if out.chars().count() + 1 >= limit {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md_link_is_plain_domain() {
        let s = "詳細は[公式ブログ](https://blog.rust-lang.org/2025/11/10/Rust-1.91.1/)参照。";
        let got = normalize_links_to_domains(s);
        // 句読点「。」は保持、Markdownリンクのみ (domain) に
        assert_eq!(got, "詳細は(blog.rust-lang.org)参照。");
    }

    #[test]
    fn raw_url_is_plain_domain() {
        let s = "URL: https://example.com/path?q=1";
        let got = normalize_links_to_domains(s);
        // プレフィックス "URL: " は保持し、URLのみ (domain) に
        assert_eq!(got, "URL: (example.com)");
    }

    #[test]
    fn zenkaku_brackets_and_spaces() {
        let s = "（参考） https://sub.example.org/a　b";
        let got = normalize_links_to_domains(s);
        // 全角括弧は半角に正規化し、注記は保持、URLは (domain) に、全角スペースは半角へ
        assert_eq!(got, "(参考) (sub.example.org) b");
    }

    #[test]
    fn fit_within_limit_keeps_bullets() {
        let s = "- 1行目\n- 2行目\n- 3行目";
        let got = fit_for_mastodon_plain(s, 12); // だいたい「- 1行目\n- 2行目」で収まる想定
        assert!(got.contains("- 1行目"));
        assert!(got.contains("- 2行目"));
        assert!(!got.contains("- 3行目"));
    }

    #[test]
    fn fit_truncates_when_no_newlines() {
        let s = "あいうえおかきくけこさしすせそたちつてと";
        let got = fit_for_mastodon_plain(s, 10);
        assert!(got.chars().count() <= 10);
        assert!(got.ends_with('…'));
    }
}
