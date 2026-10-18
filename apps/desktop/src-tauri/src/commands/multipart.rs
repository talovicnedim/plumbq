use plumbq_core::multipart::{self, MultipartField};

/// The fields of a multipart body, for the body editor.
#[tauri::command]
#[specta::specta]
pub fn multipart_parse(body: String, boundary: String) -> Vec<MultipartField> {
    multipart::parse(&body, &boundary)
}

/// The multipart body for the editor's fields.
#[tauri::command]
#[specta::specta]
pub fn multipart_format(fields: Vec<MultipartField>, boundary: String) -> String {
    multipart::format(&fields, &boundary)
}
