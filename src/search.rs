
use std::fs::File;
use std::io::prelude::*;
#[allow(unused_imports)]
use std::io::{self, BufRead};
// use std::io::Error;
#[allow(unused_imports)]
use std::path::Path;
use std::char;
use std::cmp;

use serde_json;
#[allow(unused_imports)]
use std::time::Duration;
#[allow(unused_imports)]
use tokio_timer::Timer;

use std::str;
#[allow(unused_imports)]
use std::thread;
#[allow(unused_imports)]
use std::sync::mpsc::sync_channel;
use std::fs;

use std::io::SeekFrom;
#[allow(unused_imports)]
use std::collections::HashMap;
use util;
use util::get_file_path;
use util::get_file_path_tuple;
#[allow(unused_imports)]
use std::collections::hash_map::Entry;
use fnv::FnvHashMap;
#[allow(unused_imports)]
use std::time::Instant;

#[allow(unused_imports)]
use bucket_list::BucketedScoreList;

#[allow(unused_imports)]
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::cmp::Ordering;

use persistence;

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Request {
    pub or : Option<Vec<Request>>,
    pub and : Option<Vec<Request>>,
    pub search: Option<RequestSearchPart>,
    pub boost: Option<Vec<RequestBoostPart>>
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct RequestSearchPart {
    pub path: String,
    pub term: String,
    pub levenshtein_distance: Option<u32>,
    pub starts_with: Option<String>,
    pub exact: Option<bool>,
    pub first_char_exact_match: Option<bool>
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct RequestBoostPart {
    pub path: String,
    pub boost_fun: BoostFunction,
    pub param: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum BoostFunction {
    Log10,
}

impl Default for BoostFunction {
    fn default() -> BoostFunction { BoostFunction::Log10 }
}


// pub enum CheckOperators {
//     All,
//     One
// }
// impl Default for CheckOperators {
//     fn default() -> CheckOperators { CheckOperators::All }
// }


#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Hit {
    pub id: u32,
    pub score: f32
}

// fn hits_to_array_iter<'a, I>(vals: I) -> Vec<Hit>
//     where I: Iterator<Item=(u32, f32)>
// {
//     let search_unrolled_time = util::util::debugTime::new("hits_to_array_iter");
//     let mut res:Vec<Hit> = vals.map(|(id, score)| Hit{id:id, score:score}).collect();
//     res.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal)); // Add sort by id
//     res
// }

fn hits_to_array(hits:FnvHashMap<u32, f32>) -> Vec<Hit> {
    debugTime!("hits_to_array");
    let mut res:Vec<Hit> = hits.iter().map(|(id, score)| Hit{id:*id, score:*score}).collect();
    res.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal)); // Add sort by id
    res
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DocWithHit {
    pub doc: serde_json::Value,
    pub hit: Hit
}

use std;
impl std::fmt::Display for DocWithHit {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "\n{}\t{}", self.hit.id, self.hit.score )?;
        write!(f, "\n{}", serde_json::to_string_pretty(&self.doc).unwrap() )?;
        Ok(())
    }
}

use doc_loader;
pub fn to_documents(hits: &Vec<Hit>, folder:&str) -> Vec<DocWithHit> {
    let doc_loader = doc_loader::DocLoader::new(folder, "data");
    hits.iter().map(|ref hit| {
        let doc = doc_loader.get_doc(hit.id as usize).unwrap();
        DocWithHit{doc:serde_json::from_str(&doc).unwrap(), hit:*hit.clone()}
    }).collect::<Vec<_>>()
}

pub fn search(folder:&str, request: Request, skip:usize, mut top:usize) -> Result<Vec<Hit>, SearchError>{
    infoTime!("search");
    let res = search_unrolled(folder, request)?;
    // println!("{:?}", res);
    // let res = hits_to_array_iter(res.iter());
    let res = hits_to_array(res);
    top = cmp::min(top + skip, res.len());
    Ok(res[skip..top].to_vec())
}


pub fn search_unrolled(folder:&str, request: Request) -> Result<FnvHashMap<u32, f32>, SearchError>{
    infoTime!("search_unrolled");
    if request.or.is_some() {
        Ok(request.or.unwrap().iter()
            .fold(FnvHashMap::default(), |mut acc, x| -> FnvHashMap<u32, f32> {
                acc.extend(&search_unrolled(folder, x.clone()).unwrap());
                acc
            }))
        // return Promise.all(request.or.map(req => search_unrolled(req)))
        // .then(results => results.reduce((p, c) => Object.assign(p, c)))
    }else if request.and.is_some(){
        let ands = request.and.unwrap();
        let mut and_results:Vec<FnvHashMap<u32, f32>> = ands.iter().map(|x| search_unrolled(folder, x.clone()).unwrap() ).collect(); // @Hack  unwrap forward errors

        debugTime!("and algorithm");
        let mut all_results:FnvHashMap<u32, f32> = FnvHashMap::default();
        // get shortest result
        let mut shortest = (0, std::u64::MAX);
        for (index, res) in and_results.iter().enumerate(){
            if (res.len() as u64) < shortest.1 {
                shortest =  (index, res.len() as u64);
            }
        }

        let shortest_result = and_results.swap_remove(shortest.0);
        for (k, v) in shortest_result {
            if and_results.iter().all(|ref x| x.contains_key(&k)){
                all_results.insert(k, v);
            }
        }
        // for res in &and_results {
        //     all_results.extend(res); // merge all results
        // }

        // all_results.retain(|&k, _| and_results.iter().all(|ref x| x.contains_key(&k)) );
        // all_results.retain(|k, _| and_results.iter().all(|ref x| x.contains_key(k as u64)) );
        Ok(all_results)
    }else if request.search.is_some(){
        Ok(search_raw(folder, request.search.unwrap())?)
    }else{
        Ok(FnvHashMap::default())
    }
}

#[allow(dead_code)]
fn add_boost(folder: &str, boost: &RequestBoostPart, hits : &mut FnvHashMap<u32, f32>) -> Result<(), SearchError> {
    let key = get_file_path_tuple(folder, &boost.path, ".boost.subObjId", ".boost.value");

    let boostkv_store = SupiIndexKeyValueStore::new(&key.0, &key.1);

    let boost_param = boost.param.unwrap_or(0.0);
    for (value_id, score) in hits.iter_mut() {
        if let Some(boost_value) = boostkv_store.get_value(*value_id) {
            match boost.boost_fun {
                BoostFunction::Log10 => {
                    *score += (boost_value  as f32 + boost_param).log10(); // @Temporary // @Hack // @Cleanup // @FixMe 
                }
            }
        }
    }
    Ok(())

}


#[derive(Debug)]
pub enum SearchError{
    Io(io::Error),
    MetaData(serde_json::Error),
    Utf8Error(std::str::Utf8Error)
}

impl From<io::Error> for SearchError { // Automatic Conversion
    fn from(err: io::Error) -> SearchError {SearchError::Io(err) }
}

impl From<serde_json::Error> for SearchError { // Automatic Conversion
    fn from(err: serde_json::Error) -> SearchError {SearchError::MetaData(err) }
}

impl From<std::str::Utf8Error> for SearchError { // Automatic Conversion
    fn from(err: std::str::Utf8Error) -> SearchError {SearchError::Utf8Error(err) }
}



use std::f32;

pub fn search_raw(folder:&str, mut request: RequestSearchPart) -> Result<FnvHashMap<u32, f32>, SearchError> {
    let term = util::normalize_text(&request.term);
    infoTime!("search and join to anchor");
    let mut hits = get_hits_in_field(folder, &mut request, &term)?;
    add_token_results(folder, &request.path, &mut hits);
    if hits.len() == 0 {return Ok(hits)};
    let mut next_level_hits:FnvHashMap<u32, f32> = FnvHashMap::default();
    // let mut next_level_hits:Vec<(u32, f32)> = vec![];

    let paths = util::get_steps_to_anchor(&request.path);
    info!("Joining {:?} hits from {:?} for {:?}", hits.len(), paths, term);
    for i in (0..paths.len()).rev() {
        let is_text_index = i == (paths.len() -1);
        let path_name = util::get_path_name(&paths[i], is_text_index);
        let key = get_file_path_tuple(folder, &path_name, ".valueIdToParent.valIds", ".valueIdToParent.mainIds");
        // let kv_store = SupiIndexKeyValueStore::new(&key.0, &key.1);
        // let kv_store = IndexKeyValueStore::new(&get_file_path(folder, &path_name, ".valueIdToParent.valIds") , &get_file_path(folder, &path_name, ".valueIdToParent.mainIds"));
        // trace!("kv_store: {:?}", kv_store);
        // let cache_lock = persistence::INDEX_32_CACHE.read().unwrap();// @FixMe move to get_values
        debugTime!("Joining to anchor");

        let cache_locker = persistence::INDEX_ID_TO_PARENT.read().unwrap();
        let kv_store = cache_locker.get(&key).unwrap();
        {
            debugTime!("Adding all values");
            next_level_hits.reserve(hits.len());
            for (value_id, score) in hits.iter() {
                // kv_store.add_values(*value_id, &cache_lock, *score, &mut next_level_hits);
                let ref values = kv_store[*value_id as usize];
                // let values = kv_store.get_values(*value_id, &cache_lock);
                next_level_hits.reserve(values.len());
                trace!("value_id: {:?} values: {:?} ", value_id, values);
                for parent_val_id in values {    // @Temporary
                    match next_level_hits.entry(*parent_val_id as u32) {
                        Vacant(entry) => { entry.insert(*score); },
                        Occupied(entry) => {
                            if *entry.get() < *score {
                                *entry.into_mut() = score.max(*entry.get()) + 0.1;
                            }
                        },
                    }
                }

                // for parent_val_id in values {    // @Temporary
                //     next_level_hits.place_back() <- (parent_val_id, *score);
                //     // next_level_hits.push((parent_val_id, *score));
                // }

                // for parent_val_id in values {
                //     let hit = next_level_hits.get(parent_val_id as u64);
                //     if  hit.map_or(true, |el| el == f32::NEG_INFINITY) {
                //         next_level_hits.insert(parent_val_id as u64, score);
                //     }else{
                //         next_level_hits.insert(parent_val_id as u64, score);
                //     }
                // }
            }
        }

        trace!("next_level_hits: {:?}", next_level_hits);
        debug!("{:?} hits in next_level_hits {:?}", next_level_hits.len(), &key.1);
        debugTime!("sort and dedup");

        // next_level_hits.sort_by(|a, b| a.0.cmp(&b.0));
        // next_level_hits.dedup_by_key(|i| i.0);
        // hits.clear();
        // debugTime!("insert to next level");
        // hits.reserve(next_level_hits.len());
        // for el in &next_level_hits {
        //     hits.insert(el.0, el.1);
        // }
        // next_level_hits.clear();

        // hits.extend(next_level_hits.iter());
        hits = next_level_hits;
        next_level_hits = FnvHashMap::default();
    }

    Ok(hits)
}

#[derive(Debug)]
struct OffsetInfo {
    byte_range_start: u64,
    byte_range_end: u64,
    line_offset: u64,
}

#[derive(Debug)]
struct CharOffset {
    path: String,
    chars: Vec<String>,
    // byte_offsets_start: Vec<u64>,
    // byte_offsets_end: Vec<u64>,
    // line_offsets: Vec<u64>,
}


impl CharOffset {
    fn new(path:&str) -> Result<CharOffset, SearchError> {
        persistence::load_index_64(&(path.to_string()+".char_offsets.byteOffsetsStart"))?;
        persistence::load_index_64(&(path.to_string()+".char_offsets.byteOffsetsEnd"))?;
        persistence::load_index_64(&(path.to_string()+".char_offsets.lineOffset"))?;
        let char_offset = CharOffset {
            path: path.to_string(),
            chars: util::file_as_string(&(path.to_string()+".char_offsets.chars"))?.lines().collect::<Vec<_>>().iter().map(|el| el.to_string()).collect(), // @Cleanup // @Temporary  sinlge  collect
            // byte_offsets_start: persistence::load_index_64(&(path.to_string()+".char_offsets.byteOffsetsStart"))?,
            // byte_offsets_end: persistence::load_index_64(&(path.to_string()+".char_offsets.byteOffsetsEnd"))?,
            // line_offsets: persistence::load_index_64(&(path.to_string()+".char_offsets.lineOffset"))?
        };
        trace!("Loaded CharOffset:{} ", path );
        trace!("{:?}", char_offset);
        Ok(char_offset)
    }
    fn get_char_offset_info(&self,character: &str) -> Result<OffsetInfo, usize>{
        match self.chars.binary_search(&character.to_string()) {
            Ok(index) => Ok(self.get_offset_info(index)),
            Err(nearest_index) => Ok(self.get_offset_info(nearest_index-1)),
        }
        // let char_index = self.chars.binary_search(&character.to_string()).unwrap(); // .unwrap() -> find closest offset
        // Ok(self.get_offset_info(char_index))
        // self.chars.binary_search(&character) { Ok(char_index) => this.get_offset_info(char_index),Err(_) => };
    }
    fn get_offset_info(&self, index: usize) -> OffsetInfo {
        let cache_lock = persistence::INDEX_64_CACHE.read().unwrap();
        let byte_offsets_start = cache_lock.get(&(self.path.to_string()+".char_offsets.byteOffsetsStart")).unwrap();
        let byte_offsets_end =   cache_lock.get(&(self.path.to_string()+".char_offsets.byteOffsetsEnd")).unwrap();
        let line_offsets =       cache_lock.get(&(self.path.to_string()+".char_offsets.lineOffset")).unwrap();

        trace!("get_offset_info path:{}\tindex:{}\toffsetSize: {}", self.path, index, byte_offsets_start.len());
        return OffsetInfo{byte_range_start: byte_offsets_start[index], byte_range_end: byte_offsets_end[index], line_offset: line_offsets[index]};
    }

}


//todo use cache
fn get_create_char_offset_info(folder:&str, path: &str,character: &str) -> Result<Option<OffsetInfo>, SearchError> { // @Temporary 
    let char_offset = CharOffset::new(&get_file_path(folder, &path, ""))?;
    return Ok(char_offset.get_char_offset_info(character).ok());
}

fn get_default_score(term1: &str, term2: &str) -> f32{
    return 2.0/(distance(term1, term2) as f32 + 0.2 )
}
fn get_default_score2(distance: u32) -> f32{
    return 2.0/(distance as f32 + 0.2 )
}

fn get_hits_in_field(folder:&str, mut options: &mut RequestSearchPart, term: &str) -> Result<FnvHashMap<u32, f32>, SearchError> {
    debugTime!("get_hits_in_field");
    let mut hits:FnvHashMap<u32, f32> = FnvHashMap::default();
    // let checks:Vec<Fn(&str) -> bool> = Vec::new();
    let term_chars = term.chars().collect::<Vec<char>>();
    // options.first_char_exact_match = options.exact || options.levenshtein_distance == 0 || options.starts_with.is_some(); // TODO fix

    if options.levenshtein_distance.unwrap_or(0) == 0 {
        options.exact = Some(true);
    }

    let start_char = if options.exact.unwrap_or(false) || options.levenshtein_distance.unwrap_or(0) == 0 || options.starts_with.is_some() && term_chars.len() >= 2 {
        Some(term_chars[0].to_string() + &term_chars[1].to_string())
    }
    else if options.first_char_exact_match.unwrap_or(false) { Some(term_chars[0].to_string() )
    }
    else { None };

    let value = start_char.as_ref().map(String::as_ref);

    trace!("Will Check distance {:?}", options.levenshtein_distance.unwrap_or(0) != 0);
    trace!("Will Check exact {:?}", options.exact);
    trace!("Will Check starts_with {:?}", options.starts_with);
    {
        let teh_callback = |line: &str, line_pos: u32| {
            // trace!("Checking {} with {}", line, term);
            let distance = if options.levenshtein_distance.unwrap_or(0) != 0 { Some(distance(term, line))} else { None };
            if (options.exact.unwrap_or(false) &&  line == term)
                || (distance.is_some() && distance.unwrap() <= options.levenshtein_distance.unwrap_or(0))
                || (options.starts_with.is_some() && line.starts_with(options.starts_with.as_ref().unwrap())  )
                // || (options.customCompare.is_some() && options.customCompare.unwrap(line, term))
                {
                // let score = get_default_score(term, line);
                let score = if distance.is_some() {get_default_score2(distance.unwrap())} else {get_default_score(term, line)};
                debug!("Hit: {:?}\tid: {:?} score: {:?}", line, line_pos, score);
                hits.insert(line_pos, score);
            }
        };
        let exact_search = if options.exact.unwrap_or(false) {Some(term.to_string())} else {None};
        get_text_lines(folder, &options.path, exact_search, value, teh_callback)?; // @Hack // @Cleanup // @FixMe Forward errors
    }
    debug!("{:?} hits in textindex {:?}", hits.len(), &options.path);
    trace!("hits in textindex: {:?}", hits);
    Ok(hits)

}

#[derive(Debug)]
pub struct SupiIndexKeyValueStore {
    pub path1:String,
    pub path2:String
}
use std::sync::RwLockReadGuard;
impl SupiIndexKeyValueStore {
    pub fn new(path1:&str, path2:&str) -> SupiIndexKeyValueStore {
        persistence::load_index_into_cache(&path1).unwrap();
        persistence::load_index_into_cache(&path2).unwrap();
        let new_store = SupiIndexKeyValueStore { path1: path1.to_string(), path2:path2.to_string()};
        new_store
    }
    pub fn get_value(&self, find: u32) -> Option<u32> {
        let cache_lock = persistence::INDEX_32_CACHE.read().unwrap();
        let values1 = cache_lock.get(&self.path1).unwrap();
        let values2 = cache_lock.get(&self.path2).unwrap();

        match values1.binary_search(&find) {
            Ok(pos) => { Some(values2[pos]) },
            Err(_) => {None},
        }
    }
    #[inline(always)]
    pub fn get_values(&self, find: u32, cache_lock: &RwLockReadGuard<HashMap<String, Vec<u32>>> ) -> Vec<u32> { // @FixMe return slice
        // debugTime!("get_values");
        // println!("Requesting {:?}", self.path1);
        // println!("Requesting {:?}", self.path2);
        // let cache_lock = persistence::INDEX_32_CACHE.read().unwrap();
        let values1 = cache_lock.get(&self.path1).unwrap();
        let values2 = cache_lock.get(&self.path2).unwrap();

        let mut result = Vec::new();
        match values1.binary_search(&find) {
            Ok(mut pos) => {
                let val_len = values1.len();
                while pos < val_len && values1[pos] == find{
                    result.push(values2[pos]);
                    pos+=1;
                }
            },Err(_) => {},
        }
        result
    }

    #[inline(always)]
    fn add_values(&self, find: u32, cache_lock: &RwLockReadGuard<HashMap<String, Vec<u32>>>, score:f32, target: &mut Vec<(u32, f32)> ) {
        // debugTime!("get_values");
        // println!("Requesting {:?}", self.path1);
        // println!("Requesting {:?}", self.path2);
        // let cache_lock = persistence::INDEX_32_CACHE.read().unwrap();
        let values1 = cache_lock.get(&self.path1).unwrap();
        let values2 = cache_lock.get(&self.path2).unwrap();

        // let mut result = Vec::new();
        match values1.binary_search(&find) {
            Ok(mut pos) => {
                let val_len = values1.len();
                while pos < val_len && values1[pos] == find{
                    // result.push(values2[pos]);
                    target.place_back() <- (values2[pos], score);
                    pos+=1;
                }
            },Err(_) => {},
        }
        // result
    }
}


trait TokensIndexKeyValueStore {
    fn new(path:&str) -> Self;
    fn get_parent_val_id(&self, find: u32) -> Option<u32>;
    #[inline(always)]
    fn get_parent_val_ids(&self, find: u32, cache_lock: &RwLockReadGuard<HashMap<String, Vec<u32>>>) -> Vec<u32>;
}


// fn token_kvdata_key(folder:&str, path:&str) -> (String, String) {
//     get_file_path_tuple(folder, &path, ".tokens.tokenValIds", ".tokens.parentValId")
// }

impl TokensIndexKeyValueStore for SupiIndexKeyValueStore {
    fn new(path:&str) -> Self {
        SupiIndexKeyValueStore::new(&(path.to_string()+".textindex.tokens.tokenValIds"), &(path.to_string()+".textindex.tokens.parentValId"))
    }
    fn get_parent_val_id(&self, find: u32) -> Option<u32>{ return self.get_value(find); }
    #[inline(always)]
    fn get_parent_val_ids(&self, find: u32, cache_lock: &RwLockReadGuard<HashMap<String, Vec<u32>>>) -> Vec<u32>{  return self.get_values(find, &cache_lock); }
}


fn add_token_results(folder:&str, path:&str, hits: &mut FnvHashMap<u32, f32>){
    debugTime!("add_token_results");
    let complete_path = &get_file_path(folder, &path, ".textindex.tokens.parentValId");
    let has_tokens = fs::metadata(&complete_path);// @FixMe Replace with lookup in metadata
    debug!("has_tokens {:?} {:?}", complete_path, has_tokens.is_ok());
    if has_tokens.is_err() { return; }

    // var hrstart = process.hrtime()
    // let token_kvdata: SupiIndexKeyValueStore = TokensIndexKeyValueStore::new(&get_file_path(folder, &path, "")); // @Temporary Prevent Reodering
    // let value_lengths = persistence::load_index(&get_file_path(folder, &path, ".length")).unwrap();
    // persistence::load_index_into_cache(&get_file_path(folder, &path, ".length")).unwrap();

    let cache_lock = persistence::INDEX_32_CACHE.read().unwrap();  // @Temporary Prevent Reodering
    let value_lengths = cache_lock.get(&get_file_path(folder, &path, ".length")).expect(&("Could not find ".to_string() + &get_file_path(folder, &path, ".length")) );

    let key = (get_file_path(folder, &path, ".textindex.tokens.tokenValIds"), get_file_path(folder, &path, ".textindex.tokens.parentValId"));
    let cache_locker = persistence::INDEX_ID_TO_PARENT.read().unwrap();
    let token_kvdata = cache_locker.get(&key).unwrap();

    // let mut token_hits:BucketedScoreList = FnvHashMap::default();
    // let mut token_hits = vec![];
    let mut token_hits:FnvHashMap<u32, f32> = FnvHashMap::default();
    for (value_id, _) in hits.iter() {
        // let parent_ids_for_token = token_kvdata.get_parent_val_ids(*value_id, &cache_lock);
        let ref parent_ids_for_token = token_kvdata[*value_id as usize];
        // trace!("value_id {:?}", value_id);
        // trace!("parent_ids_for_token {:?}", parent_ids_for_token);
        if parent_ids_for_token.len() > 0 {
            token_hits.reserve(parent_ids_for_token.len());
            for token_parentval_id in parent_ids_for_token {
                let parent_text_length = value_lengths[*token_parentval_id as usize];
                let token_text_length = value_lengths[*value_id as usize];
                let adjusted_score = 2.0/(parent_text_length as f32 - token_text_length as f32) + 0.2;
                // if (adjusted_score < 0) throw new Error('asdf')

                let the_score = token_hits.entry(*token_parentval_id as u32) // @Temporary
                    .or_insert(*hits.get(token_parentval_id).unwrap_or(&0.0));
                *the_score += adjusted_score;

                // let hit = hits.get(token_parentval_id as u64);
                // if  hit.map_or(true, |el| el == f32::NEG_INFINITY) { // has not key
                //     token_hits.insert(token_parentval_id as u64, adjusted_score);
                // }else{
                //     token_hits.insert(token_parentval_id as u64, adjusted_score + hit.unwrap());
                // }

                // token_hits.push(token_parentval_id);
            }
        }
    }
    debug!("checked {:?}, got {:?} token hits",hits.iter().count(), token_hits.iter().count());
    hits.extend(token_hits);
    // {
    //     debugTime!("token_hits.sort_by");
    //     token_hits.sort_by(|a, b| a.0.cmp(&b.0));
    // }
    // for hit in token_hits {
    //     hits.insert(hit, 1.5);
    // }
    // for hit in hits.iter() {
    //     trace!("NEW HITS {:?}", hit);
    // }

}


#[derive(Debug)]
struct FileAccess {
    path: String,
    // offsets: Vec<u64>,
    file: File,
    buffer: Vec<u8>
}


impl FileAccess {

    fn new(path: &str) -> Self {
        persistence::load_index_64(&(path.to_string()+".offsets")).unwrap();
        FileAccess{path:path.to_string(), file: File::open(path).unwrap(), buffer: Vec::with_capacity(50 as usize)}
    }

    fn load_text<'a>(&mut self, pos: usize, offsets:&Vec<u64>) { // @Temporary Use Result
        let string_size = offsets[pos+1] - offsets[pos] - 1;
        // let mut buffer:Vec<u8> = Vec::with_capacity(string_size as usize);
        // unsafe { buffer.set_len(string_size as usize); }
        self.buffer.resize(string_size as usize, 0);
        self.file.seek(SeekFrom::Start(offsets[pos])).unwrap();
        self.file.read_exact(&mut self.buffer).unwrap();
        // unsafe {str::from_utf8_unchecked(&buffer)}
        // let s = unsafe {str::from_utf8_unchecked(&buffer)};
        // str::from_utf8(&buffer).unwrap() // @Temporary  -> use unchecked if stable
    }

    fn binary_search(&mut self, term: &str) -> Result<(String, i64), io::Error> {
        let cache_lock = persistence::INDEX_64_CACHE.read().unwrap();
        let offsets = cache_lock.get(&(self.path.to_string()+".offsets")).unwrap();
        debugTime!("term binary_search");
        if offsets.len() < 2  {
            return Ok(("".to_string(), -1));
        }
        // let mut buffer:Vec<u8> = Vec::with_capacity(50 as usize);
        // let mut f = File::open(&self.path)?;
        let mut low = 0;
        let mut high = offsets.len() - 2;
        let mut i;
        while low <= high {
            i = (low + high) >> 1;
            self.load_text(i, offsets);
            // println!("Comparing {:?}", str::from_utf8(&buffer).unwrap());
        // comparison = comparator(arr[i], find);
            if str::from_utf8(&self.buffer).unwrap() < term { low = i + 1; continue }
            if str::from_utf8(&self.buffer).unwrap() > term { high = i - 1; continue }
            return Ok((str::from_utf8(&self.buffer).unwrap().to_string(), i as i64))
        }
        Ok(("".to_string(), -1))
    }
}

#[inline(always)]
fn get_text_lines<F>(folder:&str, path: &str, exact_search:Option<String>, character: Option<&str>, mut fun: F) -> Result<(), SearchError>
where F: FnMut(&str, u32) {

    if exact_search.is_some() {
        let mut faccess = FileAccess::new(&get_file_path(folder, &path, ""));
        let result = faccess.binary_search(&exact_search.unwrap())?;
        if result.1 != -1 {
            fun(&result.0, result.1 as u32 );
        }

    }else if character.is_some() {
        debug!("Search CharOffset for: {:?}", character.unwrap());
        let char_offset_info_opt = get_create_char_offset_info(folder, path, character.unwrap())?;
        debug!("CharOffset: {:?}", char_offset_info_opt);
        if char_offset_info_opt.is_none() {
            return Ok(())
        }
        let char_offset_info = char_offset_info_opt.unwrap();
        let mut f = File::open(&get_file_path(folder, &path, ""))?;
        let mut buffer:Vec<u8> = Vec::with_capacity((char_offset_info.byte_range_end - char_offset_info.byte_range_start) as usize);
        unsafe { buffer.set_len(char_offset_info.byte_range_end as usize - char_offset_info.byte_range_start as usize); }

        f.seek(SeekFrom::Start(char_offset_info.byte_range_start as u64))?;
        f.read_exact(&mut buffer)?;
        // let s = unsafe {str::from_utf8_unchecked(&buffer)};
        let s = str::from_utf8(&buffer)?; // @Temporary  -> use unchecked if stable
        // trace!("Loaded Text: {}", s);
        let lines = s.lines();
        let mut pos = 0;
        for line in lines{
            fun(&line, char_offset_info.line_offset as u32 + pos );
            pos += 1;
        }
        debug!("Checked {:?} lines", pos);

    }else{
        let mut f = File::open(&get_file_path(folder, &path, ""))?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        let lines = s.lines();
        for (line_pos, line) in lines.enumerate(){
            fun(&line, line_pos as u32)
        }
    }

    Ok(())
}


pub fn test_levenshtein(term:&str, max_distance:u32) -> Result<(Vec<String>), io::Error> {

    use std::time::SystemTime;
    
    let mut f = try!(File::open("de_full_2.txt"));
    let mut s = String::new();
    try!(f.read_to_string(&mut s));

    let now = SystemTime::now();

    let lines = s.lines();
    let mut hits = vec![];
    for line in lines{
        let distance = distance(term, line);
        if distance < max_distance {
            hits.push(line.to_string())
        }
    }
    
    let ms = match now.elapsed() {
        Ok(elapsed) => {(elapsed.as_secs() as f64) * 1_000.0 + (elapsed.subsec_nanos() as f64 / 1000_000.0)}
        Err(_e) => {-1.0}
    };

    let lines_checked = s.lines().count() as f64;
    println!("levenshtein ms: {}", ms);
    println!("Lines : {}", lines_checked );
    let ms_per_1000 = ((ms as f64) / lines_checked) * 1000.0;
    println!("ms per 1000 lookups: {}", ms_per_1000);
    Ok((hits))

}


fn distance(s1: &str, s2: &str) -> u32 {
    let len_s1 = s1.chars().count();

    let mut column: Vec<u32> = Vec::with_capacity(len_s1+1);
    unsafe { column.set_len(len_s1+1); }
    for x in 0..len_s1+1 {
        column[x] = x as u32;
    }

    for (x, current_char2) in s2.chars().enumerate() {
        column[0] = x as u32  + 1;
        let mut lastdiag = x as u32;
        for (y, current_char1) in s1.chars().enumerate() {
            if current_char1 != current_char2 { lastdiag+=1 }
            let olddiag = column[y+1];
            column[y+1] = cmp::min(column[y+1]+1, cmp::min(column[y]+1, lastdiag));
            lastdiag = olddiag;

        }
    }
    column[len_s1]

}

