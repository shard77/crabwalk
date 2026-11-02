use anyhow::{Context, Result};

// Changing these names would make existing keyring tokens unreachable.
const KEYRING_SERVICE: &str = "crabwalk";
const KEYRING_USER: &str = "github-pat";

fn keyring_entry() -> Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).context("creating keyring entry")
}

pub fn load_from_keyring() -> Option<String> {
    // Show token setup if the keyring is unavailable.
    let entry = keyring_entry().ok()?;
    match entry.get_password() {
        Ok(token) if !token.is_empty() => Some(token),
        _ => None,
    }
}

pub fn save_to_keyring(token: &str) -> Result<()> {
    keyring_entry()?
        .set_password(token)
        .context("saving token to keyring")
}
