#![feature(retain_hash_collection)]
#![feature(slice_get_slice)]

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate serde_json;

extern crate serde;
extern crate rand;
extern crate futures;
extern crate futures_cpupool;
extern crate tokio_timer;
extern crate regex;
extern crate fnv;
extern crate fst;

#[macro_use] extern crate log;
extern crate env_logger;

extern crate flate2;

extern crate abomonation;

#[allow(unused_imports)]
use fst::{IntoStreamer, Streamer, Levenshtein, Set, MapBuilder};
use std::fs::File;
use std::io::prelude::*;
#[allow(unused_imports)]
use std::io::{self, BufRead};
#[allow(unused_imports)]
use fnv::FnvHashSet;
#[allow(unused_imports)]
use std::collections::HashSet;
use std::collections::HashMap;
use fnv::FnvHashMap;

use std::time::Instant;

#[macro_use] extern crate lazy_static;


// extern crate rustc_serialize;
pub mod util;
pub mod search;
pub mod create;
pub mod doc_loader;

#[cfg(test)]
mod tests;


#[derive(Debug)]
struct FileAccess {
    path: String,
    curr: u64,
} 

fn main() {

    env_logger::init().unwrap();

    info!("starting up");

    // let wa = "💩";
    // println!("wa {:?}",wa.chars().count());

    // println!("{:?}",test_build_f_s_t());
    // println!("{:?}",testfst("anschauen", 2));
    // println!("{:?}",search::test_levenshtein("anschauen", 2));

    // println!("Hello, world!");
    // search::main2();
    println!("{:?}",create_index());

    let doc_loader = doc_loader::DocLoader::new("jmdict", "data");

    let now = Instant::now();
    println!("{:?}", doc_loader.get_doc(1000).unwrap());
    println!("Load Time: {}", (now.elapsed().as_secs() as f64 * 1_000.0) + (now.elapsed().subsec_nanos() as f64 / 1000_000.0));


    // println!("{:?}", testfst("寿司は焦げられない", 2));

    println!("{:?}",test_build_fst());

}
// { "fulltext":"meanings.ger[]", "options":{"tokenize":true, "stopwords": ["stopword"]} }

fn create_index() -> Result<(), io::Error> {
    let indices = r#"
    [
        {
            "boost": "commonness",
            "options": { "boost_type": "int" }
        },
        { "fulltext": "kanji[].text" },
        { "fulltext": "kana[].text" },
        {
            "fulltext": "meanings.ger[].text",
            "options": { "tokenize": true  }
        },
        {
            "boost": "meanings.ger[].rank",
            "options": { "boost_type": "int" }
        },
        {
            "fulltext": "meanings.eng[]",
            "options": { "tokenize": true  }
        },
        {
            "boost": "kanji[].commonness",
            "options": { "boost_type": "int" }
        },
        {
            "boost": "kana[].commonness",
            "options": { "boost_type": "int" }
        }
    ]
    "#;
    let mut f = File::open("jmdict.json")?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;
    println!("{:?}", create::create_indices("jmdict", &s,  indices));
    Ok(())
}



pub fn testfst(term:&str, max_distance:u32) -> Result<(Vec<String>), fst::Error> {

    let mut f = try!(File::open("de_full_2.txt"));
    let mut s = String::new();
    try!(f.read_to_string(&mut s));
    let lines = s.lines().collect::<Vec<&str>>();
    // lines.sort();

    println!("{:?}", lines.len());


    // A convenient way to create sets in memory.
    // let set = try!(Set::from_iter(lines));

    let keys = vec!["寿司は焦げられない"];
    let set = try!(Set::from_iter(keys));

    let now = Instant::now();

    let lev = try!(Levenshtein::new(term, max_distance));
    let stream = set.search(lev).into_stream();
    let hits = try!(stream.into_strs());

    println!("fst ms: {}", (now.elapsed().as_secs() as f64 * 1_000.0) + (now.elapsed().subsec_nanos() as f64 / 1000_000.0));

    // assert_eq!(hits, vec!["fo", "fob", "foo", "food"]);

    Ok((hits))
}

// fn split_at_first()  {

//     lines.sort();
//     let firsts = lines.into_iter().map(|line: &str| {
//         let splits = line.split(" ").collect::<Vec<&str>>();
//         splits[0].to_string()

//     }).collect::<Vec<String>>();
//     File::create("de_full_2.txt")?.write_all(firsts.join("\n").as_bytes());
// }

fn test_build_fst() -> Result<(), fst::Error> {
    let now = Instant::now();

    let mut f = File::open("de_full_2.txt")?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;
    let lines = s.lines().collect::<Vec<&str>>();
    println!("lines: {:?}", lines.len());


    let wtr = io::BufWriter::new(File::create("map.fst")?);
    // Create a builder that can be used to insert new key-value pairs.
    let mut build = MapBuilder::new(wtr)?;

    let mut i = 0;
    for line in lines {
        build.insert(line, i).unwrap();
        i += 1;
    }

    // println!("mapsize: {:?}", build.len());
    // println!("lines: {:?}", lines.len());
    // println(dupl_terms_checker.len())
    // Finish construction of the map and flush its contents to disk.
    build.finish()?;

    println!("test_build_fst ms: {}", (now.elapsed().as_secs() as f64 * 1_000.0) + (now.elapsed().subsec_nanos() as f64 / 1000_000.0));


    Ok(())
}


#[test]
fn it_works() {

    assert_eq!(util::normalize_text("Hello"), "Hello");
    assert_eq!(util::normalize_text("(Hello)"), "Hello");
    assert_eq!(util::normalize_text("\"H,ell-;o"), "Hello");
    assert_eq!(util::normalize_text("Hello(f)"), "Hello");
    assert_eq!(util::normalize_text("Hello(2)"), "Hello");

    assert_eq!(util::normalize_text("majestätisches Aussehen (n)"), "majestätisches Aussehen");

    assert_eq!(util::remove_array_marker("Hello[]"), "Hello");
    assert_eq!(util::remove_array_marker("Hello[].ja"), "Hello.ja");

}


extern crate bit_set;

pub fn bench_fnvhashmap_insert(num_hits: u32, token_hits: u32){
    let mut hits:FnvHashMap<u32, f32> = FnvHashMap::default();
    for x in 0..num_hits {
        hits.insert(x * 8, 0.22);
    }
    for x in 0..token_hits {
        let stat = hits.entry(x * 15 as u32).or_insert(0.0);
        *stat += 2.0;
    }
}

// pub fn bench_hashmap_insert(num_hits: u32, token_hits: u32){
//     let mut hits:HashMap<u32, f32> = HashMap::default();
//     for x in 0..num_hits {
//         hits.insert(x * 8, 0.22);
//     }
//     for x in 0..token_hits {
//         let stat = hits.entry(x * 15 as u32).or_insert(0.0);
//         *stat += 2.0;
//     }
// }

pub fn bench_fnvhashmap_extend(num_hits: u32, token_hits: u32){
    let mut hits:FnvHashMap<u32, f32> = FnvHashMap::default();
    for x in 0..num_hits {
        hits.insert(x * 8, 0.22);
    }
    let mut hits2:FnvHashMap<u32, f32> = FnvHashMap::default();
    for x in 0..token_hits {
        hits2.insert(x * 15, 0.22);
    }
    hits.extend(hits2);
}

pub fn bench_vc_scoreonly_insert(num_hits: u32, token_hits: u32){

    let mut scores:Vec<f32> = Vec::new();
    scores.resize(50, 0.0);
    for x in 0..num_hits {
        let val_id = x * 8 as u32;
        if val_id >= scores.len() as u32 {
            scores.resize((val_id as f32 * 1.5) as usize, 0.0);
        }
        scores[val_id  as usize] = 0.22;
    }
    for x in 0..token_hits {
        let val_id = x * 15 as u32;
        if val_id >= scores.len() as u32 {
            scores.resize((val_id as f32 * 1.5) as usize, 0.0);
        }
        scores[val_id as usize] += 2.0;
    }
}

pub fn bench_bucketed_insert(num_hits: u32, token_hits: u32){

    let mut scores = BucketedScoreList{arr: vec![]};
    for x in 0..num_hits {
        scores.insert((x * 8) as u64, 0.22);
    }
    for x in 0..token_hits {
        let val_id = x * 15;
        let yop = scores.get(val_id as u64).unwrap_or(&0.0) + 2.0;
        scores.insert(val_id as u64, yop);
    }
}

// pub fn bench_bit_vec_insert(){
//     let mut hits = BitSet::new();
//     let mut scores:Vec<f32> = Vec::new();
//     for x in 0..100000 {
//         hits.insert(x * 8);
//         scores.push(0.22);
//     }
//     for x in 0..100000 {
//         hits.binary_search(&(x*12 as u32));

//         let res = match hits.binary_search(&(x*12 as u32)) {
//             Ok(value) => { Some(scores[value]) },
//             Err(_) => {None},
//         };

//     }
// }


struct BucketedScoreList {
    arr: Vec<Vec<f32>>
}

use std::process;
impl BucketedScoreList {
    fn insert(& mut self, index: u64, value:f32) {
        // let bucket = (index & 0b0000000000001111) as usize;
        // let pos = (index - 1024 * bucket as u32) as usize;

        let pos = (index & 0b00000000000000000000000000001111) as usize;
        let bucket = ((index & 0b11111111111111111111111111110000) / 1024) as usize;

        if pos > index as usize {
            println!("WHAAAAT  {}", index);
            process::exit(1);
        }
        // println!("bucket {:?}" ;
        if self.arr.len() <= bucket {
            self.arr.resize(bucket+1, vec![]);
        }
        if self.arr[bucket].len() <= pos {
            self.arr[bucket].resize(((pos + 1) as f32 * 1.5) as usize, 0.0);
        }
        self.arr[bucket][pos] = value;
    }

    fn get(&self, index: u64) -> Option<&f32> {
        // let bucket = index & 0b0000000000001111;
        // let pos = index - 1024 * bucket;

        let pos = (index & 0b00000000000000000000000000001111) as usize;
        let bucket = ((index & 0b11111111111111111111111111110000) / 1024) as usize;
        if self.arr.len() <= bucket {
            None
        }else{
            self.arr[bucket].get(pos)
        }
    }
    // fn num_values(&self){
        // self.arr.iter()
        //     .fold(0, |acc2, &subArr| {
        //         acc2 + subArr.iter.fold(0, |acc, &x| {
        //             if x == 0 { acc } else { acc + 1 }
        //         })
        //     })
    // }
}

// pub fn quadratic_yes() {
//     let mut one = HashSet::new();
//     for i in 1..500000 {
//         one.insert(i);
//     }
//     let mut two = HashSet::new();
//     for v in one {
//         two.insert(v);
//     }
// }

pub fn quadratic_no(num_hits: u32) {
    let mut one = HashMap::new();
    for i in 1..num_hits {
        one.insert(i, 0.5);
    }
    let mut two = HashMap::new();
    two.extend(one);
}


// static  K100K = 100000;



// static K1K: u32 =   1000;
// static K3K: u32 =  3000;
// static K10K: u32 =  10000;
// static K100K: u32 = 100000;
// static K300K: u32 = 300000;
// static K500K: u32 = 500000;
// static K3MIO: u32 = 3000000;
// static MIO: u32 =   1000000;


// #[cfg(test)]
// mod tests {
    // use test::Bencher;

    // #[bench]
    // fn bench_fnvhashmap_insert_(b: &mut Bencher) {
    //     b.iter(|| bench_fnvhashmap_insert(K500K, K500K));
    // }

    // // #[bench]
    // // fn bench_hashmap_insert_(b: &mut Bencher) {
    // //     b.iter(|| bench_hashmap_insert(K500K, K300K));
    // // }

    // #[bench]
    // fn bench_hashmap_extend_(b: &mut Bencher) {
    //     b.iter(|| bench_fnvhashmap_extend(K500K, K500K));
    // }

    // #[bench]
    // fn bench_vec_scoreonly_insert_(b: &mut Bencher) {
    //     b.iter(|| bench_vc_scoreonly_insert(K500K, K500K));
    // }

    // // #[bench]
    // // fn quadratic_yes_(b: &mut Bencher) {
    // //     b.iter(|| quadratic_yes());
    // // }

    // #[bench]
    // fn bench_bucketed_insert_(b: &mut Bencher) {
    //     b.iter(|| bench_bucketed_insert(K500K, K500K));
    // }


    // #[bench]
    // fn quadratic_noo_(b: &mut Bencher) {
    //     b.iter(|| quadratic_no(K500K));
    // }

// }


