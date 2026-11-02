use anyhow::Result;
use arboard::Clipboard;

pub(super) fn copy_to_clipboard(text: &str) -> Result<()> {
    let text = text.to_string();
    // Open before spawning so a missing display is reported to the caller.
    let mut cb = Clipboard::new()?;

    // Linux needs a live clipboard owner to serve paste requests after this function returns.
    #[cfg(target_os = "linux")]
    {
        use arboard::SetExtLinux;
        std::thread::spawn(move || {
            let _ = cb.set().wait().text(text);
        });
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    {
        cb.set_text(text)?;
        Ok(())
    }
}
