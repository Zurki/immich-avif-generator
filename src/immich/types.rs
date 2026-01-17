use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumResponse {
    pub id: String,
    pub album_name: String,
    pub asset_count: i64,
    #[serde(default)]
    pub assets: Vec<AssetResponse>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetResponse {
    pub id: String,
    pub original_file_name: String,
    pub checksum: String,
    #[serde(rename = "type")]
    pub asset_type: AssetType,
    pub original_mime_type: Option<String>,
    pub file_size: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum AssetType {
    Image,
    Video,
    Audio,
    Other,
}

impl AssetResponse {
    pub fn is_image(&self) -> bool {
        self.asset_type == AssetType::Image
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
}
