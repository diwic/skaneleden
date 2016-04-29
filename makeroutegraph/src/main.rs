extern crate rustc_serialize;
extern crate petgraph;

use std::collections::HashMap;
use petgraph::Graph;
use petgraph::graph::NodeIndex;

#[derive(RustcDecodable, Default, Debug, Clone)]
struct StopArea {
    id: i32,
    name: String,
    x: i32,
    y: i32,
}

#[derive(Debug)]
struct Node {
    pos: (f64, f64),
    stoparea: Option<i32>,
    etapp: Option<(String, f64)>,
    node_index: NodeIndex,
}

impl Node {
    fn etapp_name(&self) -> &str { &self.etapp.as_ref().unwrap().0 }
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 { ((a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1)).sqrt() }

fn closest<'a, I: Iterator<Item=&'a Node>>(p: (f64, f64), i: I) -> (&'a Node, f64) {
    let mut r = None;
    let d = i.fold(None, |mdist, n| {
        let d = dist(p, n.pos);
        if let Some(m) = mdist { if m < d { return mdist }};
        r = Some(n);
        Some(d)
    });
    (r.unwrap(), d.unwrap())
}
/*
fn min_dist(g: &petgraph::Graph<Node, f64>, ni: u32) -> (u32, f64) {
    let node: &Node = &graph[ni];
    let mut max_dist = None;
    for (i, n) in graph.raw_nodes().enumerate() {
        if max_dist.map(
    }
}
*/

fn add_node(g: &mut Graph<Node, f64>, pos: (f64, f64), etapp: &str, d: f64) -> NodeIndex {
    let n = Node { pos: pos, stoparea: None, etapp: Some((etapp.into(), d)), node_index: 0u32.into() };
    let ni = g.add_node(n);
    let n: &mut Node = &mut g[ni];
    n.node_index = ni;
    ni
}

fn add_node2(g: &mut Graph<Node, f64>, pos: (f64, f64), sa: i32) -> NodeIndex {
    let n = Node { pos: pos, stoparea: Some(sa), etapp: None, node_index: 0u32.into() };
    let ni = g.add_node(n);
    let n: &mut Node = &mut g[ni];
    n.node_index = ni;
    ni
}

fn make_svg(graph: &Graph<Node, f64>) {
    use std::io::Write;

    let scale = 0.03f64;

    let minx = graph.raw_nodes().iter().map(|m| m.weight.pos.0 as i32).min().unwrap() as f64;
    let maxx = graph.raw_nodes().iter().map(|m| m.weight.pos.0 as i32).max().unwrap() as f64;
    let miny = graph.raw_nodes().iter().map(|m| m.weight.pos.1 as i32).min().unwrap() as f64;
    let maxy = graph.raw_nodes().iter().map(|m| m.weight.pos.1 as i32).max().unwrap() as f64;
    let xsize = maxx - minx;
    let ysize = maxy - miny;
    println!("Rect: ({},{}) to ({},{}) - total: ({}, {})", minx, miny, maxx, maxy, xsize, ysize);

    let mut f = std::fs::File::create("../fetchkoords/data/net.svg").unwrap();
    write!(f, "<svg height=\"{}\" width=\"{}\">\n", xsize * scale, ysize * scale).unwrap();
    for e in graph.raw_edges() {
        let p1 = graph[e.source()].pos;
        let p2 = graph[e.target()].pos;
        write!(f, "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" style=\"stroke:rgb({});stroke-width:2\" />\n",
            (p1.1 - miny) * scale, (maxx - p1.0) * scale, (p2.1 - miny) * scale, (maxx - p2.0) * scale,
            if graph[e.source()].stoparea.is_some() || graph[e.target()].stoparea.is_some() { "0,0,255" } else { "255,0,0" } 
            ).unwrap();   
    }
    write!(f, "</svg>\n").unwrap();
}

fn do_stop_area_work(graph: &mut Graph<Node, f64>, stopareas: HashMap<i32, StopArea>) {
    let area_to_ni: HashMap<i32, NodeIndex> = stopareas.values()
        .map(|v| (v.id, add_node2(graph, (v.x as f64, v.y as f64), v.id))).collect();
    println!("Added {} stop areas", area_to_ni.len());

    // For every point, calculate the closest stop area.
    let v: HashMap<NodeIndex, Vec<(NodeIndex, f64)>> = {
        let mut v = HashMap::new();
        let sa_nodes: Vec<&Node> = area_to_ni.values().map(|&ni| &graph[ni]).collect();
        for ni in graph.node_indices().filter(|&ni| graph[ni].etapp.is_some()) {
            let (nn, d) = closest(graph[ni].pos, sa_nodes.iter().map(|&x| x));
            if d >= 5000f64 { continue; }
            v.entry(nn.node_index).or_insert(vec!()).push((ni, d)); 
        }
        v
    };
    println!("{} stop areas are candidates", v.len());

    // TODO: For now, just add the closest link. Maybe one stop area can serve more
    // than one point, but that's probably uncommon.
    for (sa_ni, mut links) in v {
        links.sort_by(|&(_, d1), &(_, d2)| d1.partial_cmp(&d2).unwrap());
        let (ni, d) = links[0];
        graph.add_edge(sa_ni, ni, d);
        println!("Connecting {} with {} ({} m)", stopareas[&graph[sa_ni].stoparea.unwrap()].name, graph[ni].etapp_name(), d as i32);
    }

    // Remove unconnected stop areas
    graph.retain_nodes(|g, ni| g.neighbors_undirected(ni).count() >= 1);
}

fn main() {
    use std::io::Read;
    let mut f = std::fs::File::open("../fetchkoords/data/etapper.json").unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    let etapper: HashMap<String, Vec<(f64, f64)>> = rustc_serialize::json::decode(&s).unwrap();

    let mut graph = petgraph::Graph::new();

    for (k, v) in &etapper {
        let mut prevn = None;
        let mut d = 0f64;
        for &p in v {
            let newn = add_node(&mut graph, p, &k, d);
            prevn.map(|pn| {
                let dd = dist({ let dummy: &Node = &graph[pn]; dummy.pos }, p);
                d += dd;
                graph.add_edge(pn, newn, dd);
            });
            prevn = Some(newn);
        }
    }

    println!("Graph has {} nodes", graph.node_count());

    let endpoints: Vec<_> = graph.node_indices().filter(|&ni| graph.neighbors_undirected(ni).count() < 2).collect();
    for ni in endpoints {
        if graph.neighbors_undirected(ni).count() >= 2 { continue; }
        let (ng, d) = {
            let gg: &Node = &graph[ni];
            let (ng, d) = closest(gg.pos,
                graph.raw_nodes().iter().map(|nn| &nn.weight).filter(|nn| nn.etapp_name() != gg.etapp_name()));
            if d > 250f64 {
                println!("{} is not close to anything, at least {} m", gg.etapp_name(), d as i32);
                continue;
            }
            println!("add link between {} and {} ({} m)", gg.etapp_name(), ng.etapp_name(), d as i32);
            (ng.node_index, d)
        };
        graph.add_edge(ni, ng, d);
    }

    let mut f = std::fs::File::open("../fetchkoords/data/stopareas.json").unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    let stopareas: HashMap<i32, StopArea> = rustc_serialize::json::decode(&s).unwrap();
    do_stop_area_work(&mut graph, stopareas);

    make_svg(&graph);
   
}