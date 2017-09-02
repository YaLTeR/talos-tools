use std::fs::File;

use serde_yaml;

use errors::*;

const CONFIG_FILENAME: &str = "config.yaml";

#[derive(Debug, Deserialize)]
pub struct Config {
    pub split_on_return_to_nexus: bool,
    pub split_on_tetromino_doors: bool,
    pub split_on_item_unlocks: bool,
    pub split_on_sigil_collection: SigilCollectionConfig,
}

#[derive(Debug, Deserialize)]
pub struct SigilCollectionConfig {
    pub in_general: bool,
    pub in_a6: bool,
    pub in_b4: bool,
}

pub fn read_config() -> Result<Config> {
    let file =
        File::open(CONFIG_FILENAME).chain_err(|| format!("could not open {}", CONFIG_FILENAME))?;
    serde_yaml::from_reader(file).chain_err(|| format!("could not parse {}", CONFIG_FILENAME))
}
