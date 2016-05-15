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
    max_distance: i32,

    walk_speed: i32, // meters per hour

    origin_sa: StopArea,
    origin_time: TimeStamp,
    dest_sa: StopArea,
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
    fn score(&self, after: TimeStamp) -> Option<i32> {
        if self.deptime < after { return None };
        let traveltime = self.arrtime.timestamp() - self.deptime.timestamp();
        let waittime = self.deptime.timestamp() - after.timestamp();
        let s = MAX_JOURNEY_TIME_SCORE - ((traveltime * 2 + waittime) as i32);
        if s > 0 { Some(s) } else { None }
    }

    fn best(js: &[Journey], after: TimeStamp) -> Option<(i32, &Journey)> {
        let mut bscore = 0;
        let mut bj = None;
        for j in js.iter() {
            j.score(after).map(|s| { if s > bscore { bj = Some(j); bscore = s; }});
        }
        bj.map(|bj| (bscore, bj))
    }

    fn duration_as_string(&self) -> String {
        let d = self.arrtime - self.deptime;
        if d.num_hours() > 0 { format!("{} h {} min", d.num_hours(), d.num_minutes() % 60) }
        else { format!("{} min", d.num_minutes()) } 
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
    println!("Checking connections from {} to {} at {}...", from.name, to.name, deptime);
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

    println!("{} connections found", journeys.iter().filter(|j| j.score(deptime).is_some()).count());
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

fn to_km(i: i32) -> f64 { (i as f64)/1000f64 }

struct FullPath {
    origj: Journey,
    destj: Journey,
    path: utils::Path,
    score: i32,
}

fn do_search(p: &SearchParams, paths: &Vec<utils::Path>, stopareas: &HashMap<i32, StopArea>) {

    // Add paths for both directions.
    let mut paths2 = vec!();
    for v in paths.iter() {
        if v.dist < p.min_distance { continue };
        if v.dist > p.max_distance { continue };
        paths2.push(v.clone());
        let mut vv = v.clone();
        vv.reverse();
        paths2.push(vv);
    }

    // Search for origin journeys 
    let sa_origin_tocheck: HashSet<i32> = paths2.iter().map(|v| v.src).collect();
    let sa_origin_threads: Vec<(i32, _)> = sa_origin_tocheck.iter().map(|&id| {
        let sa1 = p.origin_sa.clone();
        let sa2 = stopareas[&id].clone();
        let time = p.origin_time;
        (id, std::thread::spawn(move || { ask_journeys(&sa1, &sa2, time) }))
    }).collect();
    let origin_journeys: HashMap<i32, Vec<Journey>> = sa_origin_threads.into_iter().map(|(id, th)|
        (id, th.join().unwrap())).collect();
    let origin_scores: HashMap<i32, (i32, Journey)> = 
        origin_journeys.iter().filter_map(
            |(&id, js)| Journey::best(js, p.origin_time).map(|(score, j)| (id, (score, j.clone())))
        ).collect();

    // Search for destination journeys
    let sa_dest_threads: Vec<(utils::Path, Journey, i32, _, _)> = paths2.iter()
        .filter_map(|v| origin_scores.get(&v.src).map(|&(os, ref oj)| (v, os, oj.clone())))
        .map(|(v, os, oj)| {
            let sa1 = stopareas[&v.dest].clone();
            let sa2 = p.dest_sa.clone();
            let time = oj.arrtime + chrono::Duration::seconds((v.dist as i64) * 3600i64 / (p.walk_speed as i64));
            let th = std::thread::spawn(move || { ask_journeys(&sa1, &sa2, time) });
            (v.clone(), oj, os, time, th)
        }).collect();
    let mut full_paths: Vec<FullPath> = sa_dest_threads.into_iter().filter_map(|(v, origj, origs, destdeptime, th)| {
        let destjs = th.join().unwrap();
        let (dests, destj) = if let Some(j) = Journey::best(&destjs, destdeptime) { j } else { return None };
        let totalscore = origs + dests - v.srcdist - v.destdist;
        Some(FullPath { origj: origj, destj: destj.clone(), path: v, score: totalscore })  
    }).collect();

    // Present result
    full_paths.sort_by(|v1, v2| v2.score.cmp(&v1.score));
    let mut sa_skip = HashSet::new();
    for i in full_paths {
        if sa_skip.contains(&i.path.src) || sa_skip.contains(&i.path.dest) { continue; }

        let src_name = &stopareas[&i.path.src].name;
        let dest_name = &stopareas[&i.path.dest].name;

        println!("");
        println!("Från {} till {}: minst {:.1} km", src_name, dest_name, to_km(i.path.dist));
        println!("  Res {}, från {} kl {} till {} kl {}, {} {}",
            i.origj.duration_as_string(),
            p.origin_sa.name, i.origj.deptime, src_name, i.origj.arrtime,
            i.origj.changes, if i.origj.changes == 1 {"byte"} else {"byten"});
        println!("  Gå minst {:.1} km, från {} till Skåneleden", to_km(i.path.srcdist), src_name);
        println!("  Gå {:.1} km, på {}", to_km(i.path.dist - i.path.srcdist - i.path.destdist), fix_etapp(&i.path.etapp));
        println!("  Gå minst {:.1} km, från Skåneleden till {}", to_km(i.path.destdist), dest_name);
        println!("  Res {}, från {} kl {} till {} kl {}, {} {}",
            i.destj.duration_as_string(),
            dest_name, i.destj.deptime, p.dest_sa.name, i.destj.arrtime,
            i.destj.changes, if i.destj.changes == 1 {"byte"} else {"byten"});
        sa_skip.insert(i.path.src);
        sa_skip.insert(i.path.dest);
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
    let d: i32 = args[1].parse().unwrap();
    let speed = args[2].parse().unwrap();
    let origin = ask_stop_area(&args[3]).unwrap();

    // Round to nearest second
    let otime = chrono::Local::now().naive_local().timestamp() / 1000;
    let otime = TimeStamp::from_timestamp(otime * 1000, 0);

    let sp = SearchParams { min_distance: d - 50, max_distance: d + 50, walk_speed: speed,
        origin_sa: origin.clone(), dest_sa: origin, origin_time: otime };

    do_search(&sp, &paths, &stopareas);
}
