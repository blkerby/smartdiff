use serde::{Deserialize, Deserializer, de::Error};

fn from_hex<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    usize::from_str_radix(&s, 16).map_err(D::Error::custom)
}

pub fn from_hex_words<'de, D>(deserializer: D) -> Result<Vec<u16>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let mut out: Vec<u16> = vec![];
    for word in s.split_ascii_whitespace() {
        out.push(u16::from_str_radix(word, 16).map_err(D::Error::custom)?);
    }
    Ok(out)
}

pub fn from_hex_words_u32<'de, D>(deserializer: D) -> Result<Vec<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let mut out: Vec<u32> = vec![];
    for word in s.split_ascii_whitespace() {
        out.push(u32::from_str_radix(word, 16).map_err(D::Error::custom)?);
    }
    Ok(out)
}

#[derive(Copy, Clone, Debug, Deserialize, PartialEq, Eq)]
pub enum Layer2Type {
    Layer2,
    BGData,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Screen {
    #[serde(rename = "X", deserialize_with = "from_hex")]
    pub x: usize,
    #[serde(rename = "Y", deserialize_with = "from_hex")]
    pub y: usize,
    #[serde(rename = "$value", deserialize_with = "from_hex_words")]
    pub data: Vec<u16>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Layer1 {
    #[serde(rename = "Screen")]
    pub screen: Vec<Screen>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Layer2 {
    #[serde(rename = "Screen")]
    pub screen: Vec<Screen>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LevelData {
    #[serde(rename = "Layer1")]
    pub layer_1: Layer1,
    #[serde(rename = "Layer2", default)]
    pub layer_2: Layer2,
}

#[derive(Debug, Deserialize, Default, PartialEq, Eq, Hash, Clone)]
pub struct BGDataData {
    #[serde(rename = "Type", default)]
    pub type_: String,
    #[serde(rename = "SOURCE", deserialize_with = "from_hex_words_u32", default)]
    pub source: Vec<u32>,
    #[serde(rename = "DEST", default)]
    pub dest: String,
    #[serde(rename = "SIZE", default)]
    pub size: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct BGData {
    #[serde(rename = "Data", default)]
    pub data: Vec<BGDataData>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RoomState {
    pub condition: String,
    #[serde(rename = "Arg", deserialize_with = "from_hex", default)]
    pub arg: usize,
    #[serde(rename = "GFXset", deserialize_with = "from_hex")]
    pub gfx_set: usize,
    #[serde(rename = "LevelData")]
    pub level_data: LevelData,
    #[serde(rename = "BGData")]
    pub bg_data: BGData,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RoomStateList {
    #[serde(rename = "State")]
    pub state: Vec<RoomState>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Room {
    #[serde(deserialize_with = "from_hex")]
    pub width: usize,
    #[serde(deserialize_with = "from_hex")]
    pub height: usize,
    #[serde(rename = "States")]
    pub states: RoomStateList,
}
