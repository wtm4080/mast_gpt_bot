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
