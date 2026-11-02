use crate::types::{FilterSpec, InactivitySort, QuickFilter};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

mod credentials;
mod defaults;

use credentials::load_from_keyring;
pub use credentials::save_to_keyring;
use defaults::{builtin_quick_filters, startup_preferences};

#[derive(Deserialize, Serialize)]
struct ConfigFile {
    default_label: Option<String>,
    default_author: Option<String>,
    default_min_inactive_days: Option<u64>,
    #[serde(default)]
    default_inactivity_sort: InactivitySort,
    #[serde(default)]
    default_show_drafts: bool,
    #[serde(default)]
    nerd_font: bool,
    // Missing presets use builtins; an explicitly saved empty list must stay empty.
    #[serde(default = "builtin_quick_filter_entries")]
    quick_filter: Vec<QuickFilterEntry>,
}

#[derive(Deserialize, Serialize)]
struct QuickFilterEntry {
    name: String,
    label: String,
    #[serde(default)]
    author: String,
    min_inactive_days: Option<u64>,
    #[serde(default)]
    inactivity_sort: InactivitySort,
    #[serde(default)]
    show_drafts: bool,
}

impl From<QuickFilterEntry> for QuickFilter {
    fn from(entry: QuickFilterEntry) -> Self {
        Self {
            name: entry.name,
            filter: FilterSpec {
                label: entry.label,
                author: entry.author,
                min_inactive_days: entry.min_inactive_days,
                inactivity_sort: entry.inactivity_sort,
                show_drafts: entry.show_drafts,
            },
        }
    }
}

impl From<QuickFilter> for QuickFilterEntry {
    fn from(filter: QuickFilter) -> Self {
        Self {
            name: filter.name,
            label: filter.filter.label,
            author: filter.filter.author,
            min_inactive_days: filter.filter.min_inactive_days,
            inactivity_sort: filter.filter.inactivity_sort,
            show_drafts: filter.filter.show_drafts,
        }
    }
}

impl From<&QuickFilter> for QuickFilterEntry {
    fn from(filter: &QuickFilter) -> Self {
        filter.clone().into()
    }
}

fn builtin_quick_filter_entries() -> Vec<QuickFilterEntry> {
    builtin_quick_filters()
        .into_iter()
        .map(QuickFilterEntry::from)
        .collect()
}

pub struct Config {
    pub token: Option<String>,
    pub prefs: Preferences,
    pub load_error: Option<anyhow::Error>,
}

// Derived Default is empty; Config::load supplies the startup preferences.
#[derive(Default)]
pub struct Preferences {
    pub quick_filters: Vec<QuickFilter>,
    pub default_filter: FilterSpec,
    pub nerd_font: bool,
}

impl From<ConfigFile> for Preferences {
    fn from(file: ConfigFile) -> Self {
        let quick_filters = file
            .quick_filter
            .into_iter()
            .map(QuickFilter::from)
            .collect();
        let default_filter = FilterSpec {
            label: file
                .default_label
                .unwrap_or_else(|| "S-waiting-on-review".to_string()),
            author: file.default_author.unwrap_or_default(),
            min_inactive_days: file.default_min_inactive_days,
            inactivity_sort: file.default_inactivity_sort,
            show_drafts: file.default_show_drafts,
        };
        Self {
            quick_filters,
            default_filter,
            nerd_font: file.nerd_font,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let (prefs, load_error) = match Self::read_config_file_at(&config_file_path()) {
            Ok(Some(file)) => (Preferences::from(file), None),
            Ok(None) => (startup_preferences(), None),
            Err(error) => (startup_preferences(), Some(error)),
        };

        Self {
            token: Self::find_token(),
            prefs,
            load_error,
        }
    }

    fn read_config_file_at(path: &Path) -> Result<Option<ConfigFile>> {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error).with_context(|| format!("Reading config {}", path.display()));
            }
        };
        toml::from_str(&contents)
            .map(Some)
            .with_context(|| format!("Invalid config {}", path.display()))
    }

    fn find_token() -> Option<String> {
        if let Ok(t) = std::env::var("CRABWALK_TOKEN")
            && !t.is_empty()
        {
            return Some(t);
        }
        load_from_keyring()
    }
}

fn config_file_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config")
        });
    base.join("crabwalk").join("config.toml")
}

pub fn save_config_file(prefs: &Preferences) -> Result<()> {
    save_config_file_at(&config_file_path(), prefs)
}

fn save_config_file_at(config_path: &Path, prefs: &Preferences) -> Result<()> {
    let default_filter = &prefs.default_filter;
    let file = ConfigFile {
        default_label: Some(default_filter.label.clone()),
        default_author: Some(default_filter.author.clone()),
        default_min_inactive_days: default_filter.min_inactive_days,
        default_inactivity_sort: default_filter.inactivity_sort,
        default_show_drafts: default_filter.show_drafts,
        nerd_font: prefs.nerd_font,
        quick_filter: prefs
            .quick_filters
            .iter()
            .map(QuickFilterEntry::from)
            .collect(),
    };

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).context("creating config directory")?;
    }

    let toml_str = toml::to_string(&file).context("serializing config")?;
    fs::write(config_path, toml_str).context("writing config file")
}
