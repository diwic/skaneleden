extern crate rustc_serialize;
extern crate utils;

use std::collections::HashMap;

#[derive(RustcDecodable, Default, Debug, Clone)]
struct StopArea {
    id: i32,
    name: String,
    x: i32,
    y: i32,
}

struct SearchParams {
    min_distance: i32,
    opt_distance: i32,
    max_distance: i32,
}

fn fix_etapp(s: &str) -> String {
    let mut ss = HashMap::new();
    for i in s.split(";").filter(|i| i.len() > 0) {
        let v: Vec<_> = i.split("_").collect();
        let mut e = ss.entry(v[0]).or_insert(vec!());
        e.push(v[1]);
    }
    let mut r = "".into();
    for (k, mut v) in ss {
       v.sort();
       let led = match k {
            "1" => "Kust-kustleden",
            "2" => "Nord-sydleden",
            "3" => "Ås-åsleden",
            "4" => "Österlenleden",
            "5" => "Öresundsleden",
            _ => panic!("Unknown led {}", v[0]),
        };
        r = format!("{}{} etapp {}", if r == "" { r } else { format!("{}, ", r) }, led, v.join(", "));
    }
    r
}

fn do_search(p: &SearchParams, paths: &Vec<utils::Path>, stopareas: &HashMap<i32, StopArea>) {
    let mut paths2: Vec<_> = paths.iter().filter(|v| v.dist >= p.min_distance && v.dist <= p.max_distance).collect();
    let mut sa_skip = HashMap::new();

    paths2.sort_by(|v1, v2| (v1.dist - p.opt_distance).abs().cmp(&(v2.dist - p.opt_distance).abs()));
    for i in paths2 {
        if sa_skip.contains_key(&i.src) || sa_skip.contains_key(&i.dest) { continue; }
        println!("");
        println!("Från {} till {}: minst {:.1} km", stopareas[&i.src].name, stopareas[&i.dest].name, (i.dist as f64)/1000f64);
        println!("  Gå minst {:.1} km från {} till Skåneleden", (i.srcdist as f64)/1000f64, stopareas[&i.src].name);
        println!("  Gå {:.1} km på {}", ((i.dist - i.srcdist - i.destdist) as f64)/1000f64, fix_etapp(&i.etapp));
        println!("  Gå minst {:.1} km från Skåneleden till {}", (i.destdist as f64)/1000f64, stopareas[&i.dest].name);
        sa_skip.insert(i.src, ());
        sa_skip.insert(i.dest, ());
    }
}


fn main() {
    use std::io::Read;
    let mut f = std::fs::File::open("../fetchkoords/data/stopareas.json").unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    let stopareas: HashMap<i32, StopArea> = rustc_serialize::json::decode(&s).unwrap();
    let paths = utils::read_paths();
    let args: Vec<_> = std::env::args().collect();
    let d = args[1].parse().unwrap();

    let sp = SearchParams { min_distance: d - 1000, max_distance: d + 1000, opt_distance: d };

    do_search(&sp, &paths, &stopareas);
}
