//! Bookmark helpers.

use crate::database::{now_rfc3339, Bookmark, Storage};

pub fn create_bookmark(
    storage: &Storage,
    document_id: &str,
    paragraph_index: usize,
    word_index: usize,
    note: &str,
) -> anyhow::Result<Bookmark> {
    let bm = Bookmark {
        id: uuid::Uuid::new_v4().to_string(),
        document_id: document_id.to_string(),
        paragraph_index: paragraph_index as i64,
        word_index: word_index as i64,
        note: note.to_string(),
        created_at: now_rfc3339(),
    };
    storage.add_bookmark(&bm)?;
    Ok(bm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{LibraryEntry, Storage};

    #[test]
    fn create_and_list() {
        let db = Storage::open_in_memory().unwrap();
        db.upsert_library_entry(&LibraryEntry {
            id: "d1".into(),
            title: "T".into(),
            path: "/t".into(),
            format: "TXT".into(),
            word_count: 10,
            progress_word: 0,
            last_opened: now_rfc3339(),
            completed: false,
        })
        .unwrap();
        create_bookmark(&db, "d1", 1, 5, "note").unwrap();
        assert_eq!(db.bookmarks_for("d1").unwrap().len(), 1);
    }
}
