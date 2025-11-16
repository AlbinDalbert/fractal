use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct FractalProject {
    pub meta: Meta,
    pub settings: Settings,
    pub included_files: Vec<String>,
    pub exclude_files: Vec<String>, // explude from project scope
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Meta {
    pub author: String,
    pub title: String,
    pub tags: Vec<String>,
    pub creation_date: Option<u64>,
    pub last_change_date: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub auto_include: bool,
    pub auto_linking_ignore: Vec<String>, // will not be linked to if mentioned in a file, but will still exist in fractal
}