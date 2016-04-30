extern crate rustc_serialize;

use std::collections::HashMap;
use std::io::Read;


#[derive(RustcDecodable, RustcEncodable, Default, Debug, Clone)]
pub struct StopArea {
    pub id: i32,
    pub name: String,
    pub x: i32,
    pub y: i32,
}

#[derive(RustcDecodable, RustcEncodable, Default, Debug, Clone)]
pub struct Path {
    pub dist: i32, // Total distance (incl dist from & to trail)
    pub srcdist: i32, // Distance to trail
    pub destdist: i32, // Distance from trail
    pub src: i32, // Stoparea (from)
    pub dest: i32, // Stoparea (to)
    pub etapp: String, // E g: 5_1;5_2
}

pub fn read_stopareas() -> HashMap<i32, StopArea> {
    let mut f = std::fs::File::open("../data/stopareas.json").unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    rustc_serialize::json::decode(&s).unwrap()
}

pub fn read_paths() -> Vec<Path> {
    let mut f = std::fs::File::open("../data/paths.json").unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    rustc_serialize::json::decode(&s).unwrap()
}
