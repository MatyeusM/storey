//! storey-meta: embedded file-metadata reads for tag suggestions.
//!
//! Pure local parsing (audio tags, JPEG EXIF, PNG text); never any
//! network. Anything needing the network stays an external plugin.

pub mod audio;
pub mod image;

use std::path::Path;

/// One `category = value` hint read from a file.
#[derive(Debug, Clone, PartialEq)]
pub struct MetaTag {
    pub category: String,
    pub value: String,
}

/// Read suggestion hints from `path`, dispatched by extension.
/// Unknown extensions and unreadable files yield no tags, never
/// an error.
pub fn suggest_for(path: &Path) -> Vec<MetaTag> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "mp3" | "flac" | "ogg" | "oga" | "opus" | "m4a" | "mp4" | "aac" | "wav" | "aif"
        | "aiff" | "ape" | "mpc" | "wv" => audio::read(path),
        "jpg" | "jpeg" | "tif" | "tiff" => image::exif(path),
        "png" => image::png(path),
        _ => Vec::new(),
    }
}

/// One hint, or `None` for blank values.
pub(crate) fn tag(category: &str, value: &str) -> Option<MetaTag> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(MetaTag {
        category: category.into(),
        value: value.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_types_yield_no_tags() {
        assert!(suggest_for(Path::new("notes.txt")).is_empty());
        assert!(suggest_for(Path::new("no-such-file-xyz.mp3")).is_empty());
        assert!(suggest_for(Path::new("no-such-file-xyz.jpg")).is_empty());
    }

    #[test]
    fn blank_values_are_dropped() {
        assert_eq!(tag("artist", "  "), None);
        assert_eq!(
            tag("artist", " X "),
            Some(MetaTag {
                category: "artist".into(),
                value: "X".into(),
            })
        );
    }
}
