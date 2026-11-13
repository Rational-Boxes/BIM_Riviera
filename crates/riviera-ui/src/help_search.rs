// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Riviera contributors

//! Help search (UI-HELP-002): every help page's sections, every sample's description and
//! walkthrough, and every ribbon tool, searched for all the words of a query. The search box in
//! each help page's bar asks for `riviera:help/search?q=<query>`; `riviera:help-search` asks for
//! the words in a dialog (the HTML window used without a web view has no form).
//!
//! A section is a page's text from one `<h2 id>` or `<h3 id>` to the next, so a hit opens the
//! page at its heading. Words in a title count most, then in a heading, then in the text; a
//! word matches the start of a word in the text ("stair" finds "stairs" and "stairlift").

use super::help::{PAGES, Sample, WALKTHROUGH};
use super::ribbon::{Catalog, DEFAULT};

/// The search results page's slug prefix (`riviera:help/search?q=<query>`).
pub(crate) const SEARCH: &str = "search?q=";

/// How many results the page lists.
const LIMIT: usize = 60;

/// Something found.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Hit {
    /// What kind: "Help", "Walkthrough", "Sample" or "Tool".
    pub kind: &'static str,
    pub title: String,
    /// Where it is: the page and section, or the tool's ribbon tab and panel.
    pub place: String,
    /// The link that opens it.
    pub href: String,
    /// A piece of its text round the first word found.
    pub snippet: String,
    pub score: u32,
}

/// One searchable piece of text.
struct Doc {
    kind: &'static str,
    title: String,
    heading: String,
    place: String,
    href: String,
    text: String,
}

/// The query's words, lower case.
fn words(query: &str) -> Vec<String> {
    query.split(|c: char| !c.is_alphanumeric() && c != '\'').filter(|w| !w.is_empty()).map(str::to_lowercase).collect()
}

/// How often `word` starts a word in `text` (lower case).
fn count(text: &str, word: &str) -> u32 {
    let mut n = 0;
    let mut from = 0;
    while let Some(i) = text[from..].find(word) {
        let at = from + i;
        if !text[..at].chars().next_back().is_some_and(char::is_alphanumeric) {
            n += 1;
        }
        from = at + word.len();
    }
    n
}

/// HTML as plain text: tags dropped, the common entities turned back into characters, and
/// runs of white space made one space.
pub(crate) fn plain(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut skip: Option<&str> = None;
    let mut rest = html;
    while let Some(c) = rest.chars().next() {
        if let Some(end) = skip {
            match rest.find(end) {
                Some(i) => rest = &rest[i + end.len()..],
                None => rest = "",
            }
            skip = None;
            in_tag = false;
            continue;
        }
        if c == '<' {
            let lower = rest.get(..8).unwrap_or(rest).to_ascii_lowercase();
            if lower.starts_with("<script") {
                skip = Some("</script>");
            } else if lower.starts_with("<style") {
                skip = Some("</style>");
            } else if lower.starts_with("<!--") {
                skip = Some("-->");
            }
            in_tag = true;
            // Inline tags join their text to the words round them; the others break it.
            let name: String = rest[1..].trim_start_matches('/').chars().take_while(char::is_ascii_alphanumeric).collect::<String>().to_ascii_lowercase();
            if !matches!(name.as_str(), "a" | "b" | "i" | "em" | "strong" | "code" | "kbd" | "span" | "mark" | "sub" | "sup" | "small") {
                out.push(' ');
            }
        } else if c == '>' && in_tag {
            in_tag = false;
        } else if !in_tag {
            out.push(c);
        }
        rest = &rest[c.len_utf8()..];
    }
    let out = out
        .replace("&nbsp;", " ")
        .replace("&rsaquo;", "›")
        .replace("&lsaquo;", "‹")
        .replace("&times;", "×")
        .replace("&ndash;", "–")
        .replace("&mdash;", "—")
        .replace("&rarr;", "→")
        .replace("&deg;", "°")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A page body's sections: (heading, its id, its text), the text before the first heading under
/// the page's own title.
fn sections(body: &str) -> Vec<(String, Option<String>, String)> {
    let mut out = Vec::new();
    let mut heading = String::new();
    let mut id = None;
    let mut rest = body;
    loop {
        let next = ["<h2 id=\"", "<h3 id=\""].iter().filter_map(|h| rest.find(h).map(|i| (i, *h))).min();
        let Some((i, open)) = next else {
            out.push((heading, id, plain(rest)));
            return out;
        };
        out.push((std::mem::take(&mut heading), id.take(), plain(&rest[..i])));
        let after = &rest[i + open.len()..];
        let Some(q) = after.find('"') else {
            out.push((String::new(), None, plain(after)));
            return out;
        };
        id = Some(after[..q].to_owned());
        let close = if open.starts_with("<h2") { "</h2>" } else { "</h3>" };
        let head_end = after.find(close).unwrap_or(after.len());
        let head = &after[q..head_end];
        heading = plain(head.split_once('>').map_or(head, |(_, h)| h));
        rest = after.get(head_end + close.len()..).unwrap_or("");
    }
}

/// Everything there is to search.
fn documents(samples: &[Sample]) -> Vec<Doc> {
    let mut docs = Vec::new();
    for p in PAGES.iter().filter(|p| p.slug != "index") {
        for (heading, id, text) in sections(p.body) {
            if text.is_empty() && heading.is_empty() {
                continue;
            }
            let href = match &id {
                Some(id) => format!("riviera:help/{}#{id}", p.slug),
                None => format!("riviera:help/{}", p.slug),
            };
            let place = if heading.is_empty() { p.title.to_owned() } else { format!("{} › {heading}", p.title) };
            docs.push(Doc { kind: "Help", title: p.title.to_owned(), heading, place, href, text });
        }
    }
    for s in samples {
        docs.push(Doc {
            kind: "Sample",
            title: s.title.clone(),
            heading: String::new(),
            place: format!("Sample models › {}", s.group),
            href: format!("riviera:sample/{}", s.name),
            text: s.description.clone(),
        });
        if let Some(md) = &s.walkthrough {
            docs.push(Doc {
                kind: "Walkthrough",
                title: s.title.clone(),
                heading: String::new(),
                place: format!("{}: walkthrough", s.title),
                href: format!("riviera:help/{WALKTHROUGH}{}", s.name),
                text: md.split_whitespace().collect::<Vec<_>>().join(" "),
            });
        }
    }
    // The ribbon's tools, where each is found.
    if let Ok(catalog) = Catalog::parse(DEFAULT) {
        for a in &catalog.actions {
            let places: Vec<String> = catalog
                .tabs
                .iter()
                .flat_map(|t| t.panels.iter().filter(|p| p.tools.iter().any(|x| x.action == a.id)).map(move |p| format!("{} › {}", t.label, p.label)))
                .collect();
            let menus: Vec<String> = catalog.menus.iter().filter(|m| m.items.contains(&a.id)).map(|m| format!("{} menu", m.label.replace('&', ""))).collect();
            if places.is_empty() && menus.is_empty() {
                continue;
            }
            let place = places.into_iter().chain(menus).collect::<Vec<_>>().join("; ");
            docs.push(Doc { kind: "Tool", title: a.label.clone(), heading: String::new(), place, href: format!("riviera:action/{}", a.id), text: a.help.clone() });
        }
    }
    docs
}

/// A piece of `text` round the first of `words` in it, about `width` characters long.
fn snippet(text: &str, words: &[String], width: usize) -> String {
    let lower = text.to_lowercase();
    // Lower-casing can change byte lengths; fall back to the start where it did.
    let at = if lower.len() == text.len() { words.iter().filter_map(|w| lower.find(w.as_str())).min().unwrap_or(0) } else { 0 };
    let mut start = at.saturating_sub(width / 3);
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (start + width).min(text.len());
    while !text.is_char_boundary(end) {
        end += 1;
    }
    // Whole words at both ends.
    if start > 0 {
        start = text[start..].find(' ').map_or(start, |i| start + i + 1);
    }
    if end < text.len() {
        end = text[..end].rfind(' ').filter(|&i| i > start).unwrap_or(end);
    }
    let mut s = String::new();
    if start > 0 {
        s.push('…');
    }
    s.push_str(&text[start..end]);
    if end < text.len() {
        s.push('…');
    }
    s
}

/// The results for `query`, best first.
pub(crate) fn search(query: &str, samples: &[Sample]) -> Vec<Hit> {
    let words = words(query);
    if words.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<Hit> = documents(samples)
        .into_iter()
        .filter_map(|d| {
            let (title, heading, text) = (d.title.to_lowercase(), d.heading.to_lowercase(), d.text.to_lowercase());
            let mut score = 0;
            for w in &words {
                let (t, h, x) = (count(&title, w), count(&heading, w), count(&text, w));
                if t + h + x == 0 {
                    return None;
                }
                score += 12 * t.min(1) + 6 * h.min(1) + x.min(8);
            }
            // The whole phrase, and a tool or page named by it, rank first.
            let phrase = words.join(" ");
            if words.len() > 1 && (title.contains(&phrase) || heading.contains(&phrase) || text.contains(&phrase)) {
                score += 10;
            }
            if title == phrase {
                score += 20;
            }
            let snippet = snippet(&d.text, &words, 220);
            Some(Hit { kind: d.kind, title: d.title, place: d.place, href: d.href, snippet, score })
        })
        .collect();
    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.place.cmp(&b.place)));
    hits
}

/// The query in a search link, decoded (`%xx` and `+`).
pub(crate) fn decode(q: &str) -> String {
    let bytes = q.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => match u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz"), 16) {
                Ok(b) => {
                    out.push(b);
                    i += 2;
                }
                Err(_) => out.push(b'%'),
            },
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A query encoded for a search link.
pub(crate) fn encode(q: &str) -> String {
    q.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.') { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// `text` escaped, with the query's words marked.
fn marked(text: &str, words: &[String]) -> String {
    let lower = text.to_lowercase();
    if lower.len() != text.len() {
        return super::help::escape(text);
    }
    let mut marks = vec![false; text.len()];
    for w in words {
        let mut from = 0;
        while let Some(i) = lower[from..].find(w.as_str()) {
            let at = from + i;
            for m in &mut marks[at..at + w.len()] {
                *m = true;
            }
            from = at + w.len();
        }
    }
    let mut out = String::new();
    let mut open = false;
    for (i, c) in text.char_indices() {
        if marks[i] != open {
            out.push_str(if marks[i] { "<mark>" } else { "</mark>" });
            open = marks[i];
        }
        out.push_str(&super::help::escape(&c.to_string()));
    }
    if open {
        out.push_str("</mark>");
    }
    out
}

/// The results page's body for `query`.
pub(crate) fn results_html(query: &str, samples: &[Sample]) -> String {
    let words = words(query);
    let hits = search(query, samples);
    let esc = super::help::escape;
    let mut out = format!("<h1>Search</h1>\n<p class=\"lead\">{} for <b>{}</b>{}</p>\n", match hits.len() {
        0 => "Nothing found".to_owned(),
        1 => "1 result".to_owned(),
        n => format!("{n} results"),
    }, esc(query), if hits.len() > LIMIT { format!("; the first {LIMIT} are listed") } else { String::new() });
    if hits.is_empty() {
        out.push_str("<p>Try fewer words, or the start of a word: <i>stair</i> also finds stairs and stairlifts. Every page is listed under <a href=\"riviera:help/index#contents\">Contents</a>.</p>\n");
        return out;
    }
    out.push_str("<div class=\"results\">\n");
    for h in hits.iter().take(LIMIT) {
        let run = if h.kind == "Tool" { format!(" <a class=\"run\" href=\"{}\">Run</a>", esc(&h.href)) } else { String::new() };
        let link = if h.kind == "Tool" { format!("<b>{}</b>", marked(&h.title, &words)) } else { format!("<a href=\"{}\">{}</a>", esc(&h.href), marked(&h.title, &words)) };
        out.push_str(&format!(
            "<div class=\"result\"><span class=\"kind\">{}</span> {link}{run}<div class=\"where\">{}</div><p>{}</p></div>\n",
            h.kind,
            esc(&h.place),
            marked(&h.snippet, &words)
        ));
    }
    out.push_str("</div>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_and_sections() {
        assert_eq!(plain("<p>Walls &amp; <b>doors</b></p><script>x()</script> Roof&nbsp;›"), "Walls & doors Roof ›");
        let s = sections("<p>Intro</p><h2 id=\"a\">First <code>one</code></h2><p>Alpha</p><h3 id=\"b\">Second</h3><p>Beta</p>");
        assert_eq!(s.len(), 3);
        assert_eq!(s[1], ("First one".to_owned(), Some("a".to_owned()), "Alpha".to_owned()));
        assert_eq!(s[2].1.as_deref(), Some("b"));
    }

    #[test]
    fn words_match_at_their_start() {
        assert_eq!(count("stairs and a stairlift; upstairs", "stair"), 2);
        assert_eq!(decode("curtain+wall%20grid%C3%A9"), "curtain wall gridé");
        assert_eq!(decode(&encode("grab bars & 50%")), "grab bars & 50%");
    }

    #[test]
    fn finds_pages_sections_and_tools() {
        let hits = search("curtain wall", &[]);
        assert!(!hits.is_empty());
        assert!(hits.iter().any(|h| h.kind == "Help" && h.href.starts_with("riviera:help/curtain-walls")), "{hits:#?}");
        let tool = hits.iter().find(|h| h.kind == "Tool" && h.href == "riviera:action/curtain.create").expect("the curtain wall tool");
        assert!(tool.place.contains("Architecture › Build"), "{tool:?}");
        // Every word must be found.
        assert!(search("curtain zzyzx", &[]).is_empty());
        // Tools found where they are on the ribbon.
        let pool = search("swimming pool", &[]).into_iter().find(|h| h.href == "riviera:action/ui.swimming_pool").expect("the pool tool");
        assert!(pool.place.contains("Interiors › Pools"), "{pool:?}");
        let page = results_html("grab bars", &[]);
        assert!(page.contains("<mark>grab</mark>"), "{page}");
    }
}
