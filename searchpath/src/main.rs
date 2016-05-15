extern crate rustc_serialize;
extern crate utils;
extern crate chrono;
extern crate hyper;
extern crate xml;

use std::collections::{HashMap, HashSet};

type TimeStamp = chrono::NaiveDateTime; // chrono::DateTime<chrono::FixedOffset>;

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

    origin_sa: StopArea,
    origin_time: TimeStamp,
}


fn ask_stop_area(n: &str) -> Option<StopArea> {
    let mut url = hyper::Url::parse("http://www.labs.skanetrafiken.se/v2.2/querystation.asp").unwrap();
    url.query_pairs_mut()
        .append_pair("inpPointFr", n);
    let res = hyper::Client::new().get(url).send().unwrap();
    if res.status != hyper::status::StatusCode::Ok { panic!("Open API broken: {:?}", res) }
    let x = xml::EventReader::new(res);
    let mut last_chars: Option<String> = None;
    let mut sa: StopArea = Default::default();
    for e in x {
        use xml::reader::XmlEvent::*;
        // TODO: Check that this is really a stop area, not a place etc
        match e.unwrap() {
            Characters(c) => { last_chars = Some(c) }
            EndElement { name: nn } => match &*nn.local_name {
                "Id" => { sa.id = last_chars.take().unwrap().parse().unwrap() }
                "Name" => { sa.name = last_chars.take().unwrap() }
                "X" => { sa.x = last_chars.take().unwrap().parse().unwrap() }
                "Y" => { sa.y = last_chars.take().unwrap().parse().unwrap() }
                "Point" => { return Some(sa) } 
                _ => {},
            },
            _ => { last_chars = None; }
        }
    }
    None
}


#[derive(Clone, Debug)]
struct Journey {
    deptime: TimeStamp,
    arrtime: TimeStamp,
    changes: i32,
}

const MAX_JOURNEY_TIME_SCORE: i32 = 6 * 60 * 60; // 6 hours

impl Journey {
    fn score(&self, p: &SearchParams) -> Option<i32> {
        if self.deptime < p.origin_time { return None };
        let traveltime = self.arrtime.timestamp() - self.deptime.timestamp();
        let waittime = (self.deptime.timestamp() - p.origin_time.timestamp()).abs();
        let s = MAX_JOURNEY_TIME_SCORE - ((traveltime * 2 + waittime) as i32);
        if s > 0 { Some(s) } else { None }
    }
}

fn ask_journeys(from: &StopArea, to: &StopArea, deptime: TimeStamp) -> Vec<Journey> {
    let mut url = hyper::Url::parse("http://www.labs.skanetrafiken.se/v2.2/resultspage.asp").unwrap();
    url.query_pairs_mut()
        .append_pair("inpDate", &deptime.format("%Y-%m-%d").to_string())
        .append_pair("inpTime", &deptime.format("%H:%M:%S").to_string())
        .append_pair("cmdAction", "search")
        .append_pair("selPointFr", &format!("{}|{}|0", from.name, from.id))
        .append_pair("selPointTo", &format!("{}|{}|0", to.name, to.id));
    println!("Checking connections from {} to {}...", from.name, to.name);
    let res = hyper::Client::new().get(url).send();
    let res = if let Ok(res) = res { res } else { println!("Open API broken: {:?}", res); return vec!() };
    if res.status != hyper::status::StatusCode::Ok { println!("Open API broken: {:?}", res); return vec!() }
 
    // Debug
/*    use std::io::Read;
    let mut s = String::new();
    res.read_to_string(&mut s).unwrap();
    println!("{}", s);
*/
    let x = xml::EventReader::new(res);
    let (mut at, mut dt, mut ch) = (None, None, None);
    let mut journeys = vec!();
    let mut last_chars: Option<String> = None;
    let mut inside_routelink = false;
    for e in x {
        use xml::reader::XmlEvent::*;
        use std::str::FromStr;
        // TODO: Lots of checks, e g that this is really a stop area, not a place etc
        match e.unwrap() {
            Characters(c) => { last_chars = Some(c) },
            StartElement { name: nn, attributes: _, namespace: _ } => match &*nn.local_name {
                "RouteLinks" => { inside_routelink = true; }
                _ => {},
            },
            EndElement { name: nn } => match &*nn.local_name {
                "RouteLinks" => inside_routelink = false,
                "ArrDateTime" => { if !inside_routelink { at = Some(last_chars.take().unwrap()) }},
                "DepDateTime" => { if !inside_routelink { dt = Some(last_chars.take().unwrap()) }},
                "NoOfChanges" => { if !inside_routelink { ch = Some(last_chars.take().unwrap()) }},
                "Journey" => { 
                    journeys.push(Journey {
                        changes: ch.unwrap().parse().unwrap(),
                        arrtime: chrono::NaiveDateTime::from_str(&at.unwrap()).unwrap(),
                        deptime: chrono::NaiveDateTime::from_str(&dt.unwrap()).unwrap() });
                    at = None; dt = None; ch = None;
                },
                _ => {},
            },
            _ => { last_chars = None; }
        }
    }

    println!("{} connections found", journeys.len());// iter().filter(|j| j.score().is_some()).count());

    //if journeys.len() > 0 { panic!("{:?}", journeys); }
    journeys
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
    let paths2: Vec<_> = paths.iter().filter(|v| v.dist >= p.min_distance && v.dist <= p.max_distance).collect();
    let mut sas_tocheck = HashSet::new();
    for i in &paths2 { sas_tocheck.insert(i.src); sas_tocheck.insert(i.dest); } 
    let sa_journeys: HashMap<_,_> = sas_tocheck.iter().map(|&id|
        (id, ask_journeys(&p.origin_sa, &stopareas[&id], p.origin_time))).collect();
    
    let mut sa_origin_scores: HashMap<i32, (i32, Journey)> = HashMap::new();
    for (&id, js) in sa_journeys.iter() {
        let mut bscore = 0;
        let mut bj = None;
        for j in js.iter() {
            j.score(p).map(|s| { if s > bscore { bj = Some(j); bscore = s; }});
        }
        bj.map(|bj| { sa_origin_scores.insert(id, (bscore, bj.clone())) });
    }

    let mut sa_skip = HashMap::new();

    let mut paths3: Vec<_> = paths2.iter().filter(|v|
        sa_origin_scores.get(&v.src).and_then(|_| sa_origin_scores.get(&v.dest)).is_some()).collect();
    paths3.sort_by(|v2, v1| std::cmp::max(sa_origin_scores[&v1.src].0, sa_origin_scores[&v1.dest].0)
        .cmp(&std::cmp::min(sa_origin_scores[&v2.src].0, sa_origin_scores[&v2.dest].0)));
   // paths2.sort_by(|v1, v2| (v1.dist - p.opt_distance).abs().cmp(&(v2.dist - p.opt_distance).abs()));
    for i in paths3 {
        if sa_skip.contains_key(&i.src) || sa_skip.contains_key(&i.dest) { continue; }

        let orig_j = &sa_origin_scores[&i.src].1;
        let orig_jtime = orig_j.arrtime - orig_j.deptime;
 
        println!("");
        println!("Från {} till {}: minst {:.1} km", stopareas[&i.src].name, stopareas[&i.dest].name, (i.dist as f64)/1000f64);
        println!("  Res {}{} min, från {} kl {} till {} kl {}, {} {}",
            if orig_jtime.num_hours() > 0 {format!("{} h ", orig_jtime.num_hours())} else {"".into()}, orig_jtime.num_minutes() % 60, 
            p.origin_sa.name, orig_j.deptime, stopareas[&i.src].name, orig_j.arrtime,
            orig_j.changes, if orig_j.changes == 1 {"byte"} else {"byten"});
        println!("  Gå minst {:.1} km, från {} till Skåneleden", (i.srcdist as f64)/1000f64, stopareas[&i.src].name);
        println!("  Gå {:.1} km, på {}", ((i.dist - i.srcdist - i.destdist) as f64)/1000f64, fix_etapp(&i.etapp));
        println!("  Gå minst {:.1} km, från Skåneleden till {}", (i.destdist as f64)/1000f64, stopareas[&i.dest].name);
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
    let origin = ask_stop_area(&args[2]).unwrap();

    let sp = SearchParams { min_distance: d - 50, max_distance: d + 50, opt_distance: d,
        origin_sa: origin, origin_time: chrono::Local::now().naive_local() };

    do_search(&sp, &paths, &stopareas);
}
