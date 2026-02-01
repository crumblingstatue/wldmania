use {
    directories::ProjectDirs,
    recently_used_list::RecentlyUsedList,
    serde::{Deserialize, Serialize},
    std::path::PathBuf,
};

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub recent_files: RecentlyUsedList<PathBuf>,
    #[serde(default)]
    pub load_most_recent: bool,
    #[serde(default)]
    pub draw_center_marker: bool,
    #[serde(default)]
    pub load_tiles_at_start: bool,
}

impl Config {
    pub fn load_or_default() -> anyhow::Result<Self> {
        let cfg_path = cfg_path();
        if cfg_path.exists() {
            let text = std::fs::read_to_string(&cfg_path)?;
            Ok(serde_json::from_str(&text)?)
        } else {
            Ok(Default::default())
        }
    }
    pub fn save(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(cfg_path().parent().unwrap())?;
        Ok(std::fs::write(
            cfg_path(),
            serde_json::to_string_pretty(self)?,
        )?)
    }
}

fn cfg_path() -> PathBuf {
    let proj_dir = ProjectDirs::from("", "crumblingstatue", "wldview").unwrap();

    proj_dir.config_dir().join("wldview.json")
}
