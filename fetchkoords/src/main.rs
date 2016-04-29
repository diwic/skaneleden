extern crate xml;
extern crate rustc_serialize;

fn wgs84_to_rt90(lat_deg: f64, lon_deg: f64) -> (f64, f64) {
    // References:
    // http://www.lantmateriet.se/globalassets/kartor-och-geografisk-information/gps-och-matning/geodesi/formelsamling/gauss_conformal_projection.pdf 
    // http://www.lantmateriet.se/sv/Kartor-och-geografisk-information/GPS-och-geodetisk-matning/Om-geodesi/Transformationer/RT-90---SWEREF-99/
    let f = 1f64/298.257222101f64;
    let lon0_deg = 15f64 + 48f64/60f64 + 22.624306f64/3600f64;
    let lon0 = lon0_deg * std::f64::consts::PI / 180f64;
    let a = 6378137f64;
    let k0 = 1.00000561024f64;
    let ffnn = -667.711f64;
    let ffee = 1500064.274f64;

    let e_2 = f * (2f64 - f);
    let n = f / (2f64 - f);
    let a_caret = a * (1f64 + n * n / 4f64 + n * n * n * n / 64f64) / (1f64 + n);

    let aa = e_2;
    let e_4 = e_2 * e_2;
    let bb = (5f64 * e_4 - e_4 * e_2)/6f64;
    let cc = (104f64 * e_4 * e_2 - 45f64 * e_4 * e_4)/120f64;
    let dd = 1237f64 * e_4 * e_4 / 1260f64;

    let beta1 = n / 2f64 - 2f64 * n * n / 3f64 + 5f64 * n * n * n / 16f64 + 41f64 * n * n * n * n / 180f64;
    let beta2 = 13f64 * n * n / 48f64 - 3f64 * n * n * n / 5f64 + 557f64 * n * n * n * n / 1440f64;
    let beta3 = 61f64 * n * n * n / 240f64 - 103f64 * n * n * n * n / 140f64;
    let beta4 = 49561f64 * n * n * n * n / 161280f64;

    let lat = lat_deg * std::f64::consts::PI / 180f64;
    let lon = lon_deg * std::f64::consts::PI / 180f64;

    let ls_2 = lat.sin() * lat.sin();
    let conf_lat = lat - lat.sin() * lat.cos() * (aa + bb * ls_2 + cc * ls_2 * ls_2 + dd * ls_2 * ls_2 * ls_2);

    let lons = lon - lon0;
    let xip = (conf_lat.tan() / lons.cos()).atan();
    let etap = (conf_lat.cos() * lons.sin()).atanh();

    let xp = xip + beta1 * (2f64 * xip).sin() * (2f64 * etap).cosh() + beta2 * (4f64 * xip).sin() * (4f64 * etap).cosh()
         + beta3 * (6f64 * xip).sin() * (6f64 * etap).cosh() + beta4 * (8f64 * xip).sin() * (8f64 * etap).cosh();
    let yp = etap + beta1 * (2f64 * xip).cos() * (2f64 * etap).sinh() + beta2 * (4f64 * xip).cos() * (4f64 * etap).sinh()
         + beta3 * (6f64 * xip).cos() * (6f64 * etap).sinh() + beta4 * (8f64 * xip).cos() * (8f64 * etap).sinh();
    let x = k0 * a_caret * xp + ffnn;
    let y = k0 * a_caret * yp + ffee;
    (x, y)
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 { ((a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1)).sqrt() }


fn process_file(fname: &std::path::Path) -> Result<(String, Vec<(f64, f64)>), Box<std::error::Error>> {
    use std::io::Read;
    println!("Processing {}", fname.to_str().unwrap());
    let mut f = try!(std::fs::File::open(fname));
    let mut s = vec!();
    try!(f.read_to_end(&mut s));
    let ss = try!(std::str::from_utf8(if &s[..3] == &[0xef, 0xbb, 0xbf][..] { &s[3..] } else { &s }));
    let r = xml::EventReader::from_str(ss);
    let mut last_char = None;
    let mut trackname = None;
    let mut points = vec!();
    for event in r {
        use xml::reader::XmlEvent::*;
        let e = try!(event);
        // println!("{:?}", e);
        match e {
            StartElement { name: nn, attributes: attr, namespace: _ } => {
                if nn.local_name != "trkpt" { continue };
                let (mut lat, mut lon): (Option<f64>, Option<f64>) = (None, None);
                for a in attr {
                    if a.name.local_name == "lat" { lat = Some(try!(a.value.parse())); }
                    if a.name.local_name == "lon" { lon = Some(try!(a.value.parse())); }
                }
                if lat.is_some() && lon.is_some() { points.push((lat.unwrap(), lon.unwrap())); }
            },
            EndElement { name: nn } => {
                if nn.local_name == "name" { trackname = last_char.take(); }
            },
            Characters (s) => { last_char = Some(s); },
            _ => {},
        }
    }
    println!("{} points found on track {}", points.len(), trackname.as_ref().unwrap());
    let rt90: Vec<_> = points.iter().map(|&(lat, lon)| wgs84_to_rt90(lat, lon)).collect();
    let mut totaldist = 0f64;
    for i in 1..rt90.len() { totaldist += dist(rt90[i], rt90[i-1]) };
    println!("Total distance: {}", totaldist);

    //println!("all: {:?}", rt90);
/*    let destfname = std::path::PathBuf::from("./data/rt90").join(fname.with_extension("rt90").file_name().unwrap());
    println!("Writing to {}", destfname.to_str().unwrap());
    let mut f = try!(std::fs::File::create(&destfname));
    for (x, y) in rt90 {
        try!(write!(f, "{} {}\n", x, y));
    } */
    Ok((trackname.unwrap(), rt90))
}

fn main() {
    use std::io::Write;
    println!("{:?}", std::env::current_dir());
    let mut b = std::collections::BTreeMap::new();
    for f in std::fs::read_dir("./data/all_gpx").unwrap() {
        let _ = f.map(|f| process_file(&f.path()).map(|(s, v)| b.insert(s, v))
            .map_err(|e| println!("{:?}", e))).map_err(|e| println!("{:?}", e));
    }
    write!(std::fs::File::create("./data/etapper.json").unwrap(), "{}", rustc_serialize::json::encode(&b).unwrap()).unwrap();
}
