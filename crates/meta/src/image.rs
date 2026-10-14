//! JPEG EXIF (rexif) plus PNG text-chunk reads.

use crate::{MetaTag, tag};

/// Make/model/date hints from EXIF; failures mean no tags.
pub fn exif(path: &std::path::Path) -> Vec<MetaTag> {
    let Ok(data) = rexif::parse_file(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in &data.entries {
        let hint = match entry.tag.to_string().as_str() {
            "Make" => tag("make", &entry.value_more_readable),
            "Model" => tag("model", &entry.value_more_readable),
            "DateTime" => tag("date", &exif_date(&entry.value_more_readable)),
            _ => None,
        };
        if let Some(hint) = hint {
            out.push(hint);
        }
    }
    out
}

/// `2024:01:02 03:04:05` becomes view-safe `2024-01-02`.
fn exif_date(raw: &str) -> String {
    raw.split(' ').next().unwrap_or(raw).replace(':', "-")
}

/// Title/author/description hints from PNG text chunks.
pub fn png(path: &std::path::Path) -> Vec<MetaTag> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(reader) = png::Decoder::new(std::io::BufReader::new(file)).read_info() else {
        return Vec::new();
    };
    let info = reader.info();
    let mut out = Vec::new();
    for chunk in &info.uncompressed_latin1_text {
        push_text(&mut out, &chunk.keyword, &chunk.text);
    }
    for chunk in &info.utf8_text {
        if let Ok(text) = chunk.get_text() {
            push_text(&mut out, &chunk.keyword, &text);
        }
    }
    out
}

/// Map one PNG keyword onto a tag category; others are ignored.
fn push_text(out: &mut Vec<MetaTag>, keyword: &str, text: &str) {
    let category = match keyword.to_ascii_lowercase().as_str() {
        "title" => "title",
        "author" => "artist",
        "description" | "comment" => "comment",
        "copyright" => "copyright",
        "software" => "software",
        _ => return,
    };
    if let Some(hint) = tag(category, text) {
        out.push(hint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_map_to_categories() {
        let mut out = Vec::new();
        push_text(&mut out, "Title", "Sunset");
        push_text(&mut out, "Author", "Ann");
        push_text(&mut out, "Creation Time", "today");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].category, "title");
        assert_eq!(out[1].category, "artist");
    }

    #[test]
    fn exif_dates_turn_view_safe() {
        assert_eq!(exif_date("2024:01:02 03:04:05"), "2024-01-02");
    }

    #[test]
    fn missing_files_yield_no_tags() {
        let missing = std::path::Path::new("/no-such-meta-xyz.png");
        assert!(png(missing).is_empty());
    }

    #[test]
    fn png_text_chunks_become_hints() {
        let dir = std::env::temp_dir().join(format!("storey-meta-png-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.png");
        let file = std::fs::File::create(&path).unwrap();
        let mut enc = png::Encoder::new(file, 1, 1);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.add_text_chunk("Title".into(), "Sunset".into()).unwrap();
        let mut writer = enc.write_header().unwrap();
        writer.write_image_data(&[0, 0, 0]).unwrap();
        drop(writer);
        let hints = png(&path);
        assert!(
            hints
                .iter()
                .any(|h| h.category == "title" && h.value == "Sunset")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
