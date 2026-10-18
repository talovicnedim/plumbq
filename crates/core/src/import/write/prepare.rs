//! Fixes what a source can hold but a `.http` file can't, and checks every request before the
//! first file is written, so an import never stops halfway.

use crate::Result;
use crate::http_file::{self, Request};
use crate::import::ImportedItem;

/// Uppercases methods (Postman allows `get`), leaves out headers without a valid name with a
/// note, and fails on a request that still can't be written.
pub(super) fn prepare(items: &mut [ImportedItem], notes: &mut Vec<String>) -> Result<()> {
    for item in items {
        match item {
            ImportedItem::Request(request) => prepare_request(request, notes)?,
            ImportedItem::Folder { items, .. } => prepare(items, notes)?,
        }
    }
    Ok(())
}

fn prepare_request(request: &mut Request, notes: &mut Vec<String>) -> Result<()> {
    request.method = request.method.trim().to_ascii_uppercase();
    let before = request.headers.len();
    request
        .headers
        .retain(|header| http_file::is_header_name(&header.name));
    let dropped = before - request.headers.len();
    if dropped > 0 {
        notes.push(format!(
            "{}: left out {dropped} headers without a valid name",
            request.name
        ));
    }
    http_file::validate(request)
        .map_err(|err| crate::Error::Invalid(format!("can't import \"{}\": {err}", request.name)))
}
