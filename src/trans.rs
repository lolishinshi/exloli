use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::read_to_string;

pub static TRANS: Lazy<Database> = Lazy::new(Database::new);

#[derive(Deserialize)]
pub struct Database {
    // head: (),
    // version: u8,
    // repo: String,
    data: Vec<Data>,
}

#[derive(Deserialize)]
pub struct Data {
    namespace: String,
    // count: i32,
    data: HashMap<String, Info>,
}

#[derive(Deserialize)]
pub struct Info {
    name: String,
    // intro: String,
    // links: String,
}

impl Database {
    fn new() -> Self {
        let text = read_to_string("db.text.json").expect("Cannot open db.text.json");
        serde_json::from_slice(text.as_bytes()).expect("Cannot parse translation database")
    }

    pub fn trans<'a>(&'a self, namespace: &'a str, name: &'a str) -> &'a str {
        for data in &self.data {
            if data.namespace == namespace {
                return data.trans(name);
            }
        }
        for data in &self.data {
            let trans = data.trans(name);
            if trans != name {
                return trans;
            }
        }
        return name;
    }
}

impl Data {
    pub fn trans<'a>(&'a self, name: &'a str) -> &'a str {
        self.data
            .get(name)
            .map(|info| info.name.as_str())
            .unwrap_or(name)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let database: Database = serde_json::from_slice(include_bytes!("../db.text.json")).unwrap();
        println!("{:?}", database.trans("female", "lolicon"));
        println!("{:?}", database.trans("female", "foobar"));
    }
}
