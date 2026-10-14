//! Audio tag reads via lofty (ID3, Vorbis, MP4 ilst, …).

use lofty::file::TaggedFileExt;
use lofty::tag::Accessor;

use crate::{MetaTag, tag};

/// Artist/album/title/genre hints; unreadable files mean no tags.
pub fn read(path: &std::path::Path) -> Vec<MetaTag> {
    let Ok(file) = lofty::read_from_path(path) else {
        return Vec::new();
    };
    let Some(found) = file.primary_tag().or_else(|| file.first_tag()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    push(&mut out, "artist", found.artist().as_deref());
    push(&mut out, "album", found.album().as_deref());
    push(&mut out, "title", found.title().as_deref());
    push(&mut out, "genre", found.genre().as_deref());
    if let Some(date) = found.date() {
        let text = stamp(&date);
        push(&mut out, "date", Some(&text));
    }
    out
}

/// Push one optional value as a hint.
fn push(out: &mut Vec<MetaTag>, category: &str, value: Option<&str>) {
    if let Some(value) = value
        && let Some(hint) = tag(category, value)
    {
        out.push(hint);
    }
}

/// `YYYY[-MM[-DD]]` from what the tag carries (view-safe by shape).
fn stamp(date: &lofty::tag::items::Timestamp) -> String {
    match (date.month, date.day) {
        (Some(month), Some(day)) => format!("{:04}-{month:02}-{day:02}", date.year),
        _ => format!("{:04}", date.year),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_files_yield_no_tags() {
        let missing = std::path::Path::new("/no-such-meta-xyz.mp3");
        assert!(read(missing).is_empty());
    }

    #[test]
    fn stamps_use_what_the_tag_carries() {
        let year_only = lofty::tag::items::Timestamp {
            year: 2001,
            month: None,
            day: None,
            hour: None,
            minute: None,
            second: None,
        };
        assert_eq!(stamp(&year_only), "2001");
        let full = lofty::tag::items::Timestamp {
            month: Some(5),
            day: Some(15),
            ..year_only
        };
        assert_eq!(stamp(&full), "2001-05-15");
    }
    // ponytail: no committed audio fixture (real FLACs are 10-50MB);
    // positive path verified by hand against local/*.flac (gitignored).
}
