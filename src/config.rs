//! Глобальная конфигурация OpenIPA: каталоги, формат вывода,жим сборки.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub home: PathBuf,
    pub sdks_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub build_dir: PathBuf,
    pub packages_dir: PathBuf,
    pub output_format: OutputFormat,
    pub verbose: bool,
    pub color: bool,
}

impl Config {
    pub fn load() -> Result<Self> {
        let home = dirs::home_dir()
            .map(|h| h.join(".openipa"))
            .unwrap_or_else(|| PathBuf::from(".openipa"));
        Ok(Self {
            sdks_dir: home.join("sdks"),
            cache_dir: home.join("cache"),
            build_dir: PathBuf::from("build"),
            packages_dir: home.join("packages"),
            home,
            output_format: OutputFormat::Human,
            verbose: false,
            color: true,
        })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        for dir in [
            &self.home,
            &self.sdks_dir,
            &self.cache_dir,
            &self.packages_dir,
        ] {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("не удалось создать каталог {}", dir.display()))?;
        }
        Ok(())
    }

    pub fn sdk_registry_path(&self) -> PathBuf {
        self.sdks_dir.join("registry.json")
    }

    pub fn build_root(&self, project: &Path) -> PathBuf {
        project.join(&self.build_dir)
    }
}
