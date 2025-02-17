/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::collections::HashMap;
use std::fmt::Debug;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tokio::fs;
use uuid::Uuid;

use crate::error::Result;
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct State {
    #[serde(default)]
    keys_to_uid: HashMap<String, String>,
}

impl State {
    pub async fn get(state_dir: &Path) -> Result<State> {
        let state_file = state_dir.join("state.json");
        match fs::read_to_string(state_file).await {
            Ok(data) => Ok(serde_json::from_str(&data)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e)?,
        }
    }

    pub fn get_item_id(&mut self, key: &String) -> String {
        if !self.keys_to_uid.contains_key(key) {
            let uid = Uuid::new_v4().to_string();
            self.keys_to_uid.insert(key.clone(), uid.clone());
            uid
        } else {
            self.keys_to_uid.get(key).unwrap().clone()
        }
    }

    pub fn get_existing_item_id(&self, key: &String) -> Option<String> {
        if !self.keys_to_uid.contains_key(key) {
            None
        } else {
            Some(self.keys_to_uid.get(key).unwrap().clone())
        }
    }

    pub async fn save(&self, state_dir: &Path) -> Result<()> {
        fs::write(state_dir.join("state.json"), serde_json::to_string(self)?).await?;
        Ok(())
    }
}
