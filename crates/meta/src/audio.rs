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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_files_yield_no_tags() {
        let missing = std::path::Path::new("/no-such-meta-xyz.mp3");
        assert!(read(missing).is_empty());
    }
    // ponytail: no positive audio fixture (hand-built MP3/FLAC rejected
    // by lofty's parser); verify against a real tagged file by hand.
}
