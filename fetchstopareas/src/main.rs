extern crate rustc_serialize;
extern crate hyper;
extern crate xml;

use std::collections::HashMap;

#[derive(RustcEncodable, Default, Debug, Clone)]
struct StopArea {
    id: i32,
    name: String,
    x: i32,
    y: i32,
}

fn ask_stop_area(x: i32, y: i32, m: &mut HashMap<i32, StopArea>) {
    let uri = format!("http://www.labs.skanetrafiken.se/v2.2/neareststation.asp?x={}&y={}&radius={}", x, y, 5000); 
    let res = hyper::Client::new().get(&uri).send().unwrap();
    if res.status != hyper::status::StatusCode::Ok { panic!("Open API broken: {:?}", res) }
    let x = xml::EventReader::new(res);
    let mut last_chars: Option<String> = None;
    let mut sa: StopArea = Default::default();
    for e in x {
        use xml::reader::XmlEvent::*;
        match e.unwrap() {
            Characters(c) => { last_chars = Some(c) }
            EndElement { name: nn } => match &*nn.local_name {
                "Id" => { sa.id = last_chars.take().unwrap().parse().unwrap() }
                "Name" => { sa.name = last_chars.take().unwrap() }
                "X" => { sa.x = last_chars.take().unwrap().parse().unwrap() }
                "Y" => { sa.y = last_chars.take().unwrap().parse().unwrap() }
                "NearestStopArea" => { m.insert(sa.id, sa.clone()); sa = Default::default(); } 
                _ => {},
            },
            _ => { last_chars = None; }
        }
    }
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 { ((a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1)).sqrt() }

fn main() {
    use std::io::{Read, Write};
    let mut f = std::fs::File::open("../fetchkoords/data/etapper.json").unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    let q: HashMap<String, Vec<(f64, f64)>> = rustc_serialize::json::decode(&s).unwrap();
    let mut m = HashMap::new();
    for (n, v) in q {
        println!("Etapp: {}", n);
        ask_stop_area(v[v.len()-1].0 as i32, v[v.len()-1].1 as i32, &mut m);
        ask_stop_area(v[0].0 as i32, v[0].1 as i32, &mut m);
        let mut last_point = v[0];
        for p in v {
            if dist(last_point, p) < 1000f64 { continue; }
            ask_stop_area(p.0 as i32, p.1 as i32, &mut m);
            last_point = p;
        }
    }

    write!(std::fs::File::create("../fetchkoords/data/stopareas.json").unwrap(), "{}",
        rustc_serialize::json::encode(&m).unwrap()).unwrap();
    
}
