/// Copy text to the system clipboard.
pub fn set_text(text: String) -> Result<(), String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|err| format!("Clipboard unavailable: {}", err))?;
    clipboard
        .set_text(text)
        .map_err(|err| format!("Clipboard write failed: {}", err))
}
