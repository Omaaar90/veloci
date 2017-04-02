#![feature(retain_hash_collection)]
#![feature(test)]

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate serde_json;

extern crate serde;
extern crate rand;
extern crate tokio_timer;
extern crate regex;
extern crate fnv;
extern crate fst;

#[macro_use] extern crate log;
extern crate env_logger;

extern crate abomonation;
extern crate csv;

extern crate test;

// use fst::{IntoStreamer, Streamer, Levenshtein, Set, MapBuilder};
#[allow(unused_imports)]
use fst::{IntoStreamer, Levenshtein, Set, MapBuilder};
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
pub mod persistence;
pub mod bucket_list;

#[cfg(test)]
mod tests;


use std::str;



fn main() {
    env_logger::init().unwrap();

    // let all_terms = loadcsv("./data.csv", 0);
    // println!("{:?}", all_terms.len());

    // File::create("MATNR").unwrap().write_all(all_terms.join("\n").as_bytes()).unwrap();
    // let indices = r#"
    // [
    //     { "fulltext":"MATNR", "attr_pos" : 0 },
    //     { "fulltext":"ISMTITLE", "attr_pos" : 1, "options":{"tokenize":true}},
    //     { "fulltext":"ISMORIGTITLE", "attr_pos" : 2, "options":{"tokenize":true}},
    //     { "fulltext":"ISMSUBTITLE1", "attr_pos" : 3, "options":{"tokenize":true}},
    //     { "fulltext":"ISMSUBTITLE2", "attr_pos" : 4, "options":{"tokenize":true}},
    //     { "fulltext":"ISMSUBTITLE3", "attr_pos" : 5, "options":{"tokenize":true}},
    //     { "fulltext":"ISMARTIST", "attr_pos" : 6, "options":{"tokenize":true}},
    //     { "fulltext":"ISMLANGUAGES", "attr_pos" : 7},
    //     { "fulltext":"ISMPUBLDATE", "attr_pos" : 8},
    //     { "fulltext":"EAN11", "attr_pos" : 9},
    //     { "fulltext":"ISMORIDCODE", "attr_pos" : 10}
    // ]
    // "#;

    // let indices = r#"
    // [
    //     { "fulltext":"MATNR", "attr_pos" : 0 },
    //     { "fulltext":"ISMTITLE", "attr_pos" : 1, "options":{"tokenize":true}}
    // ]
    // "#;

    let indices = r#"
    [
        { "fulltext":"MATNR", "attr_pos" : 0 },
        { "fulltext":"ISMTITLE", "attr_pos" : 1}
    ]
    "#;
    // println!("{:?}", create::create_indices_csv("csv_test", "./data.csv", indices));
    let meta_data = persistence::MetaData::new("csv_test");
    println!("{:?}", persistence::load_all(&meta_data));

    {
        println!("Ab gehts");
        let my_time = util::MeasureTime::new("search total");
        // let req = json!({
        //     "or":[
        //         {
        //             "and":[{"search": {"term":"kriege", "path": "MATNR"}}, {"search": {"term":"die", "path": "MATNR"}}, {"search": {"term":"ich", "path": "MATNR"}}, {"search": {"term":"gesehen", "path": "MATNR"}}, {"search": {"term":"habe", "path": "MATNR"}} ]
        //         },
        //         {
        //             "and":[{"search": {"term":"kriege", "path": "ISMTITLE"}}, {"search": {"term":"die", "path": "ISMTITLE"}}, {"search": {"term":"ich", "path": "ISMTITLE"}}, {"search": {"term":"gesehen", "path": "ISMTITLE"}}, {"search": {"term":"habe", "path": "ISMTITLE"}} ]
        //         },
        //         {
        //             "and":[{"search": {"term":"kriege", "path": "ISMORIGTITLE"}}, {"search": {"term":"die", "path": "ISMORIGTITLE"}}, {"search": {"term":"ich", "path": "ISMORIGTITLE"}}, {"search": {"term":"gesehen", "path": "ISMORIGTITLE"}}, {"search": {"term":"habe", "path": "ISMORIGTITLE"}} ]
        //         },
        //         {
        //             "and":[{"search": {"term":"kriege", "path": "ISMSUBTITLE1"}}, {"search": {"term":"die", "path": "ISMSUBTITLE1"}}, {"search": {"term":"ich", "path": "ISMSUBTITLE1"}}, {"search": {"term":"gesehen", "path": "ISMSUBTITLE1"}}, {"search": {"term":"habe", "path": "ISMSUBTITLE1"}} ]
        //         },
        //         {
        //             "and":[{"search": {"term":"kriege", "path": "ISMSUBTITLE2"}}, {"search": {"term":"die", "path": "ISMSUBTITLE2"}}, {"search": {"term":"ich", "path": "ISMSUBTITLE2"}}, {"search": {"term":"gesehen", "path": "ISMSUBTITLE2"}}, {"search": {"term":"habe", "path": "ISMSUBTITLE2"}} ]
        //         },
        //         {
        //             "and":[{"search": {"term":"kriege", "path": "ISMSUBTITLE3"}}, {"search": {"term":"die", "path": "ISMSUBTITLE3"}}, {"search": {"term":"ich", "path": "ISMSUBTITLE3"}}, {"search": {"term":"gesehen", "path": "ISMSUBTITLE3"}}, {"search": {"term":"habe", "path": "ISMSUBTITLE3"}} ]
        //         },
        //         {
        //             "and":[{"search": {"term":"kriege", "path": "ISMARTIST"}}, {"search": {"term":"die", "path": "ISMARTIST"}}, {"search": {"term":"ich", "path": "ISMARTIST"}}, {"search": {"term":"gesehen", "path": "ISMARTIST"}}, {"search": {"term":"habe", "path": "ISMARTIST"}} ]
        //         },
        //         {
        //             "and":[{"search": {"term":"kriege", "path": "ISMLANGUAGES"}}, {"search": {"term":"die", "path": "ISMLANGUAGES"}}, {"search": {"term":"ich", "path": "ISMLANGUAGES"}}, {"search": {"term":"gesehen", "path": "ISMLANGUAGES"}}, {"search": {"term":"habe", "path": "ISMLANGUAGES"}} ]
        //         },
        //         {
        //             "and":[{"search": {"term":"kriege", "path": "ISMPUBLDATE"}}, {"search": {"term":"die", "path": "ISMPUBLDATE"}}, {"search": {"term":"ich", "path": "ISMPUBLDATE"}}, {"search": {"term":"gesehen", "path": "ISMPUBLDATE"}}, {"search": {"term":"habe", "path": "ISMPUBLDATE"}} ]
        //         },
        //         {
        //             "and":[{"search": {"term":"kriege", "path": "EAN11"}}, {"search": {"term":"die", "path": "EAN11"}}, {"search": {"term":"ich", "path": "EAN11"}}, {"search": {"term":"gesehen", "path": "EAN11"}}, {"search": {"term":"habe", "path": "EAN11"}} ]
        //         },
        //         {
        //             "and":[{"search": {"term":"kriege", "path": "ISMORIDCODE"}}, {"search": {"term":"die", "path": "ISMORIDCODE"}}, {"search": {"term":"ich", "path": "ISMORIDCODE"}}, {"search": {"term":"gesehen", "path": "ISMORIDCODE"}}, {"search": {"term":"habe", "path": "ISMORIDCODE"}} ]
        //         }
        //     ]
        // });

        // let req = json!({
        //     "and":[{"search": {"term":"kriege", "path": "ISMTITLE"}}, {"search": {"term":"die", "path": "ISMTITLE"}}, {"search": {"term":"ich", "path": "ISMTITLE"}}, {"search": {"term":"gesehen", "path": "ISMTITLE"}}, {"search": {"term":"habe", "path": "ISMTITLE"}} ]
        // });

        let req = json!({
            "search": {"term":"kriege die ich gesehen habe", "path": "ISMTITLE"}
        });

        let requesto: search::Request = serde_json::from_str(&req.to_string()).unwrap();
        let hits = search::search("csv_test", requesto, 0, 10).unwrap();
        println!("{:?}", hits);
    }


    // {
    //     let my_time = util::MeasureTime::new("binary_search total");
    //     let mut faccess = FileAccess::new("jmdict/meanings.ger[].text");
    //     let result = faccess.binary_search("haus");
    //     let result = faccess.binary_search("genau");
    //     let result = faccess.binary_search("achtung");
    //     // println!("{:?}", result);
    // }

    // println!("{:?}",test_build_f_s_t());
    // println!("{:?}",testfst("anschauen", 2));
    // println!("{:?}",search::test_levenshtein("anschauen", 2));

    // println!("{:?}",create_index());


    // let _ = env_logger::init();
    // let req = json!({
    //     "search": {
    //         "term":"haus",
    //         "path": "meanings.ger[].text",
    //         "levenshtein_distance": 0,
    //         "firstCharExactMatch":true
    //     }
    // });

    // let requesto: search::Request = serde_json::from_str(&req.to_string()).unwrap();
    // let my_time = util::MeasureTime::new("Search");
    // let hits = search::search("jmdict", requesto, 0, 10).unwrap();

    // let requesto2: search::Request = serde_json::from_str(&req.to_string()).unwrap();
    // let hits2 = search::search("jmdict", requesto2, 0, 10).unwrap();

    // let docs = search::to_documents(&hits, "jmdict");

    // println!("{:?}", hits);




    // let doc_loader = doc_loader::DocLoader::new("jmdict", "data");
    // let now = Instant::now();
    // println!("{:?}", doc_loader.get_doc(1000).unwrap());
    // println!("Load Time: {}", (now.elapsed().as_secs() as f64 * 1_000.0) + (now.elapsed().subsec_nanos() as f64 / 1000_000.0));

    // println!("{:?}",test_build_fst());

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


// #[test]
// fn it_works() {

//     assert_eq!(util::normalize_text("Hello"), "hello");
//     assert_eq!(util::normalize_text("(Hello)"), "hello");
//     assert_eq!(util::normalize_text("\"H,ell-;o"), "hello");
//     assert_eq!(util::normalize_text("Hello(f)"), "hello");
//     assert_eq!(util::normalize_text("Hello(2)"), "hello");

//     assert_eq!(util::normalize_text("majestätisches Aussehen (n)"), "majestätisches aussehen");

//     assert_eq!(util::remove_array_marker("Hello[]"), "hello");
//     assert_eq!(util::remove_array_marker("Hello[].ja"), "hello.ja");

// }
