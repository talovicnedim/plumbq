use plumbq_core::compare::{self, BodyComparison};

/// Field-by-field (JSON) or line-by-line comparison of two response bodies. Bodies can be big,
/// so it runs off the main thread.
#[tauri::command(async)]
#[specta::specta]
pub fn compare_bodies(before: String, after: String) -> BodyComparison {
    compare::bodies(&before, &after)
}
