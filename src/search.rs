#[allow(unused_imports)]
use std::io::{self, BufRead};
#[allow(unused_imports)]
use std::path::Path;
use std::cmp;

use std;
#[allow(unused_imports)]
use std::{str, thread, f32};
#[allow(unused_imports)]
use std::sync::mpsc::sync_channel;

#[allow(unused_imports)]
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::cmp::Ordering;

#[allow(unused_imports)]
use fnv::FnvHashMap;
use serde_json;
#[allow(unused_imports)]
use std::time::Duration;
#[allow(unused_imports)]
use itertools::Itertools;

// use search_field;
use persistence::Persistence;
use doc_loader::DocLoader;
use util;
use util::*;
use util::concat;
use fst;
use fst_levenshtein;

use search_field::*;
#[allow(unused_imports)]
use test;
#[allow(unused_imports)]
use execution_plan;
use execution_plan::*;
// use execution_plan::execute_plan;

#[allow(unused_imports)]
use rayon::prelude::*;
#[allow(unused_imports)]
use crossbeam_channel;
#[allow(unused_imports)]
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Request {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub or: Option<Vec<Request>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub and: Option<Vec<Request>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<RequestSearchPart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggest: Option<Vec<RequestSearchPart>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boost: Option<Vec<RequestBoostPart>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boost_term: Option<Vec<RequestSearchPart>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facets: Option<Vec<FacetRequest>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default = "default_top")]
    pub top: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default = "default_skip")]
    pub skip: Option<usize>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct FacetRequest {
    pub field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default = "default_top")]
    pub top: Option<usize>,
}

fn default_top() -> Option<usize> {Some(10) }
fn default_skip() -> Option<usize> {None }

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct RequestSearchPart {
    pub path: String,
    pub terms: Vec<String>,                             //TODO only first term used currently
    #[serde(default = "default_term_operator")]
    pub term_operator: TermOperator,                    //TODO unused currently

    #[serde(skip_serializing_if = "Option::is_none")]
    pub levenshtein_distance: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub starts_with: Option<bool>,

    #[serde(default)]
    pub ids_only: bool,                                 //TODO unused currently

    /// Also return the actual text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_term: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_value: Option<RequestBoostPart>,

    /// boosts the search part with this value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boost: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default = "default_resolve_token_to_parent_hits")]
    pub resolve_token_to_parent_hits: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<usize>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip: Option<usize>,

    /// return the snippet hit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<bool>,

    /// Override default SnippetInfo
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet_info: Option<SnippetInfo>,

    /// Internal data
    #[serde(default)]
    pub fast_field: bool,
}
impl PartialEq for RequestSearchPart {
    fn eq(&self, other: &RequestSearchPart) -> bool {
        format!("{:?}", self) == format!("{:?}", other)
    }
}
impl Eq for RequestSearchPart {}

impl PartialOrd for RequestSearchPart {
    fn partial_cmp(&self, other: &RequestSearchPart) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RequestSearchPart {
    fn cmp(&self, other: &RequestSearchPart) -> Ordering {
        format!("{:?}", self).cmp(&format!("{:?}", other))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SnippetInfo {
    #[serde(default = "default_num_words_around_snippet")]
    pub num_words_around_snippet: i64,
    #[serde(default = "default_snippet_start")]
    pub snippet_start_tag: String,
    #[serde(default = "default_snippet_end")]
    pub snippet_end_tag: String,
    #[serde(default = "default_snippet_connector")]
    pub snippet_connector: String,
    #[serde(default = "default_max_snippets")]
    pub max_snippets: u32,
}

fn default_num_words_around_snippet() -> i64 { 5 }
fn default_snippet_start() -> String { "<b>".to_string() }
fn default_snippet_end() -> String { "</b>".to_string() }
fn default_snippet_connector() -> String { " ... ".to_string() }
fn default_max_snippets() -> u32 { std::u32::MAX }

lazy_static! {
    pub static ref DEFAULT_SNIPPETINFO: SnippetInfo = SnippetInfo{
        num_words_around_snippet :  default_num_words_around_snippet(),
        snippet_start_tag: default_snippet_start(),
        snippet_end_tag: default_snippet_end(),
        snippet_connector: default_snippet_connector(),
        max_snippets: default_max_snippets(),
    };
}

fn default_resolve_token_to_parent_hits() -> Option<bool> {
    Some(true)
}

fn default_term_operator() -> TermOperator {
    TermOperator::ALL
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TermOperator {
    ALL,
    ANY,
}
impl Default for TermOperator {
    fn default() -> TermOperator {
        default_term_operator()
    }
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct RequestBoostPart {
    pub path: String,
    pub boost_fun: Option<BoostFunction>,
    pub param: Option<f32>,
    pub skip_when_score: Option<Vec<f32>>,
    pub expression: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum BoostFunction {
    Log10,
    Linear,
    Add,
}

impl Default for BoostFunction {
    fn default() -> BoostFunction {
        BoostFunction::Log10
    }
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct SearchResult {
    pub num_hits: u64,
    pub data: Vec<Hit>,
    pub ids: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facets: Option<FnvHashMap<String, Vec<(String, usize)>>>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct SearchResultWithDoc {
    pub num_hits: u64,
    pub data: Vec<DocWithHit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facets: Option<FnvHashMap<String, Vec<(String, usize)>>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DocWithHit {
    pub doc: serde_json::Value,
    pub hit: Hit,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub struct Hit {
    pub id: u32,
    pub score: f32,
}

impl Hit {
    pub fn new(id: u32, score: f32) -> Hit {
        Hit { id, score }
    }
}

impl From<(u32, f32)> for Hit {
    fn from(tupl: (u32, f32)) -> Self {
        Hit::new(tupl.0, tupl.1)
    }
}

// #[cfg_attr(feature = "flame_it", flame)]
// fn hits_to_sorted_array(hits: FnvHashMap<u32, f32>) -> Vec<Hit> {
//     //TODO add top n sort
//     debug_time!("hits_to_sorted_array");
//     let mut res: Vec<Hit> = hits.iter().map(|(id, score)| Hit { id: *id, score: *score }).collect();
//     res.sort_unstable_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal)); //TODO Add sort by id when equal
//     res
// }

impl std::fmt::Display for DocWithHit {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "\n{}\t{}", self.hit.id, self.hit.score)?;
        write!(f, "\n{}", serde_json::to_string_pretty(&self.doc).unwrap())?;
        Ok(())
    }
}

// @FixMe Tests should use to_search_result
#[cfg_attr(feature = "flame_it", flame)]
pub fn to_documents(persistence: &Persistence, hits: &[Hit]) -> Vec<DocWithHit> {
    // DocLoader::load(persistence);
    hits.iter()
        .map(|ref hit| {
            let doc = DocLoader::get_doc(persistence, hit.id as usize).unwrap();
            DocWithHit {
                doc: serde_json::from_str(&doc).unwrap(),
                hit: *hit.clone(),
            }
        })
        .collect::<Vec<_>>()
}

#[cfg_attr(feature = "flame_it", flame)]
pub fn to_search_result(persistence: &Persistence, hits: SearchResult) -> SearchResultWithDoc {
    SearchResultWithDoc {
        data: to_documents(&persistence, &hits.data),
        num_hits: hits.num_hits,
        facets: hits.facets,
    }
}

use search_field;
use fixedbitset::FixedBitSet;

#[inline]
pub fn to_bucket_and_id(value: u32) -> (u16, u16) {
    ((value >> 16) as u16, value as u16)
}

#[cfg_attr(feature = "flame_it", flame)]
pub fn search(mut request: Request, persistence: &Persistence) -> Result<SearchResult, SearchError> {
    info_time!("search");
    request.top = request.top.or(Some(10));
    request.skip = request.skip;
    // let skip = request.skip.unwrap_or(0);
    // let top = request.top.unwrap_or(10);

    let plan = plan_creator(request.clone());
    let yep = plan.get_output();
    plan.execute_step(persistence)?;
    // execute_step(plan, persistence)?;
    let mut res = yep.recv()?;
    drop(yep);

    // let res = search_unrolled(&persistence, request)?;
    // println!("{:?}", res);
    // let res = hits_to_array_iter(res.iter());
    // let res = hits_to_sorted_array(res);

    let mut search_result = SearchResult {
        num_hits: 0,
        data: vec![],
        ids: vec![],
        facets: None,
    };

    {
        // let boosto = persistence.term_boost_cache.clone();
        if let Some(boost_term) = request.boost_term {
            info_time!("boost_term");
            {
                persistence.term_boost_cache.write().get(&boost_term);//poke
            }

            let mut from_cache = false;
            // Attentión - The read lock is still help in the else block therefore we need to create an extra scope to avoid deadlocks
            // This should be probably fixed sometime with better lifetime handling in rust
            {
                if let Some(data) = persistence.term_boost_cache.read().peek(&boost_term) {
                    // let mut boost_iter = data.hits_ids.iter().map(|el|el.clone());
                    // res = apply_boost_from_iter(res, &mut boost_iter)
                    info_time!("boost_term_cache");
                    let mut boost_iter = data.iter()
                        .map(|el| {
                            let boost_val:f32 = el.request.boost.unwrap_or(2.0).clone();
                            el.hits_ids.iter().map(move|id| Hit::new(*id, boost_val ))
                        })
                        .into_iter().kmerge_by(|a, b| a.id < b.id);

                    {

                        let mut boost_iter_data:Vec<Hit> = data.iter()
                        .map(|el| {
                            let boost_val:f32 = el.request.boost.unwrap_or(2.0).clone();
                            el.hits_ids.iter().map(move|id| Hit::new(*id, boost_val ))
                        })
                        .into_iter().kmerge_by(|a, b| a.id < b.id).collect();

                        {
                            info_time!("binary search boost");
                            let mut last_pos = 0;
                            for hit in res.hits_vec.iter_mut(){
                                match boost_iter_data[last_pos ..].binary_search_by_key(&hit.id, |&hit| hit.id) {
                                    Ok(boost_hit) => {
                                        hit.score *= boost_iter_data[boost_hit].score;
                                        last_pos =boost_hit;
                                    },
                                    Err(pos) => { last_pos = pos;},
                                }
                            }
                        }

                        {
                            let mut direct_data:Vec<f32> = vec![];
                            for hit in boost_iter_data.iter() {
                                if direct_data.len() <= hit.id as usize {
                                    direct_data.resize(hit.id as usize + 1, 0.0);
                                }
                                direct_data[hit.id as usize] = hit.score;
                            }
                            info_time!("direct search boost");
                            for hit in res.hits_vec.iter_mut(){
                                if let Some(boost_hit) = direct_data.get(hit.id as usize) {
                                    hit.score *= boost_hit;
                                }
                            }
                        }

                        {
                            let my_boost = 2.0;
                            println!("SIZE {:?}", boost_iter_data.last().unwrap().id);
                            let mut direct_data:FixedBitSet = {

                                let mut ay = FixedBitSet::with_capacity(70000 as usize + 1);
                                for hit in boost_iter_data.iter() {
                                    let (_, id_in_bucket) = to_bucket_and_id(hit.id);
                                    ay.insert(id_in_bucket as usize);
                                }
                                ay
                            };
                            info_time!("direct search bitset");
                            for hit in res.hits_vec.iter_mut(){
                                let (_, id_in_bucket) = to_bucket_and_id(hit.id);
                                if direct_data.contains(id_in_bucket as usize) {
                                    hit.score *= my_boost;
                                }
                            }
                        }

                        // { // Hashmap ist doof
                        //     let mut boost_iter_data:FnvHashMap<u32, f32> = data.iter()
                        //     .map(|el| {
                        //         let boost_val:f32 = el.request.boost.unwrap_or(2.0).clone();
                        //         el.hits_ids.iter().map(move|id| Hit::new(*id, boost_val ))
                        //     })
                        //     .into_iter().kmerge_by(|a, b| a.id < b.id).map(|hit| (hit.id, hit.score)).collect();

                        //     info_time!("hashmap boost");
                        //     for hit in res.hits_vec.iter_mut(){
                        //         if let Some(boost_hit) = boost_iter_data.get(&hit.id) {
                        //             hit.score *= boost_hit;
                        //         }
                        //     }
                        // }

                        {
                            info_time!("merge search boost");
                            res = apply_boost_from_iter(res, &mut boost_iter_data.into_iter());
                        }

                        debug_time!("binary search".to_string());

                    }

                    debug_time!("boost_intersect_hits_vec_multi".to_string());
                    res = apply_boost_from_iter(res, &mut boost_iter);


                    from_cache = true;

                }
            }

            if !from_cache{
                let r: Result<Vec<_>, SearchError> = boost_term.clone().into_par_iter()
                    .map(|mut boost_term_req| {
                        boost_term_req.ids_only = true;
                        boost_term_req.fast_field = true;
                        search_field::get_hits_in_field(persistence, boost_term_req, None)
                    })
                    .collect();
                let mut data = r?;
                res = boost_intersect_hits_vec_multi(res, &mut data);
                {
                    persistence.term_boost_cache.write().insert(boost_term.clone(), data);
                }

            }

        }
    }

    // print!("{:?}", res.hits_vec);
    if res.hits_vec.len() > 0 {
        //TODO extract only top n
        res.hits_vec.sort_unstable_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal)); //TODO Add sort by id when equal
        search_result.data = res.hits_vec;
    }
    search_result.num_hits = search_result.data.len() as u64;

    if let Some(facets_req) = request.facets {
        let mut hit_ids: Vec<u32> = {
            debug_time!("get_and_sort_for_factes");
            let mut hit_ids: Vec<u32> = search_result.data.iter().map(|el| el.id).collect();
            hit_ids.sort_unstable();
            hit_ids
        };
        search_result.facets = Some(
            facets_req
                .par_iter()
                .map(|facet_req| (facet_req.field.to_string(), facet::get_facet(persistence, facet_req, &hit_ids).unwrap()))
                .collect(),
        );
    }
    search_result.data = apply_top_skip(search_result.data, request.skip, request.top);

    Ok(search_result)
}

//TODO no copy
#[cfg_attr(feature = "flame_it", flame)]
pub fn apply_top_skip<T: Clone>(hits: Vec<T>, skip: Option<usize>, top: Option<usize>) -> Vec<T> {
    let skip = skip.unwrap_or(0);
    if let Some(mut top) = top {
        top = cmp::min(top + skip, hits.len());
        hits[skip..top].to_vec()
    } else {
        hits[skip..].to_vec()
    }
    // top = cmp::min(top + skip, hits.len());
    // hits[skip..top].to_vec()
}


use facet;

#[cfg_attr(feature = "flame_it", flame)]
pub fn get_shortest_result<T: std::iter::ExactSizeIterator>(results: &Vec<T>) -> usize {
    let mut shortest = (0, std::u64::MAX);
    for (index, res) in results.iter().enumerate() {
        if (res.len() as u64) < shortest.1 {
            shortest = (index, res.len() as u64);
        }
    }
    shortest.0
}

#[cfg_attr(feature = "flame_it", flame)]
pub fn get_longest_result<T: std::iter::ExactSizeIterator>(results: &Vec<T>) -> usize {
    let mut longest = (0, std::u64::MIN);
    for (index, res) in results.iter().enumerate() {
        if (res.len() as u64) > longest.1 {
            longest = (index, res.len() as u64);
        }
    }
    longest.0
}

// #[cfg_attr(feature = "flame_it", flame)]
// pub fn union_hits(mut or_results: Vec<SearchFieldResult>) -> SearchFieldResult {
//     let index_longest = get_longest_result(&or_results.iter().map(|el| el.hits.iter()).collect());
//     let longest_result = or_results.swap_remove(index_longest).hits;

//     let mut result = SearchFieldResult::default();
//     result.hits = longest_result;

//     let estimate_additional_elements: usize = or_results.iter().map(|el| el.hits.len()).sum();
//     result.hits.reserve(estimate_additional_elements / 2);

//     for res in or_results {
//         result.hits.extend(&res.hits);
//     }

//     result
// }

#[cfg_attr(feature = "flame_it", flame)]
pub fn union_hits_vec(mut or_results: Vec<SearchFieldResult>) -> SearchFieldResult {
    if or_results.len() == 1 {
        return or_results.swap_remove(0);
    }

    let index_longest = get_longest_result(&or_results.iter().map(|el| el.hits_vec.iter()).collect());

    let longest_len = or_results[index_longest].hits_vec.len() as f32;
    let len_total: usize = or_results.iter().map(|el| el.hits_vec.len()).sum();
    let sum_other_len = len_total as f32 - longest_len;

    // if longest_len as f32 * 0.05 > sum_other_len{ // TODO check best value
    //     let mut union_hits = or_results.swap_remove(index_longest).hits_vec;

    //INSERT SUPER SLOW
    // {
    //     debug_time!("union hits sort input".to_string());
    //     for res in or_results.iter_mut() {
    //         res.hits_vec.sort_unstable_by_key(|el| el.id);
    //         //TODO ALSO DEDUP???
    //     }
    // }

    // let iterators:Vec<_> = or_results.iter().map(|el| el.hits_vec.iter()).collect();
    // let mergo = iterators.into_iter().kmerge_by(|a, b| a.id < b.id);
    // debug_time!("union hits kmerge".to_string());

    // for (mut id, mut group) in &mergo.into_iter().group_by(|el| el.id)
    // {
    //     let sum_score = group.map(|a| a.score).sum(); // TODO same term = MAX, different terms = SUM
    //     let mkay = union_hits.binary_search_by_key(&id, |&a| a.id);
    //     match mkay {
    //         Ok(pos) => {
    //             union_hits[pos].score += sum_score;
    //         },
    //         Err(pos) => {
    //             union_hits.insert(pos, Hit::new(id,sum_score))
    //         },
    //     }
    // }

    //     {
    //         debug_time!("union hits append ".to_string());
    //         for mut res in or_results {
    //             union_hits.append(&mut res.hits_vec);
    //         }
    //     }

    //     debug_time!("union hits sort and dedup ".to_string());
    //     union_hits.sort_unstable_by_key(|el| el.id);
    //     let prev = union_hits.len();
    //     union_hits.dedup_by_key(|el| el.id); // TODO FixMe Score

    //     debug!("union hits merged from {} to {} hits", prev, union_hits.len() );

    //     SearchFieldResult {
    //         hits_vec: union_hits,
    //         ..Default::default()
    //     }
    // }else{

    {
        debug_time!("union hits sort input".to_string());
        for res in or_results.iter_mut() {
            res.hits_vec.sort_unstable_by_key(|el| el.id);
            //TODO ALSO DEDUP???
        }
    }

    let iterators: Vec<_> = or_results.iter().map(|el| el.hits_vec.iter()).collect();

    let mut union_hits = Vec::with_capacity(longest_len as usize + sum_other_len as usize / 2);
    let mergo = iterators.into_iter().kmerge_by(|a, b| a.id < b.id);

    debug_time!("union hits kmerge".to_string());

    for (mut id, mut group) in &mergo.into_iter().group_by(|el| el.id) {
        // let best_score = group.max_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(Ordering::Equal)).unwrap().score;
        // union_hits.push(Hit::new(id,best_score));
        let sum_score = group.map(|a| a.score).sum(); // TODO same term = MAX, different terms = SUM
        union_hits.push(Hit::new(id, sum_score));
    }

    // debug!("union hits merged from {} to {} hits", prev, union_hits.len() );
    SearchFieldResult {
        hits_vec: union_hits,
        ..Default::default()
    }
    // }
}

#[test]
fn union_hits_vec_test() {
    let hits1 = vec![Hit::new(10, 20.0), Hit::new(0, 10.0), Hit::new(5, 20.0)]; // unsorted
    let hits2 = vec![Hit::new(0, 20.0), Hit::new(3, 20.0), Hit::new(10, 30.0), Hit::new(20, 30.0)];

    let yop = vec![
        SearchFieldResult {
            hits_vec: hits1,
            ..Default::default()
        },
        SearchFieldResult {
            hits_vec: hits2,
            ..Default::default()
        },
    ];

    let res = union_hits_vec(yop);
    println!("{:?}", res.hits_vec);

    assert_eq!(
        res.hits_vec,
        vec![Hit::new(0, 30.0), Hit::new(3, 20.0), Hit::new(5, 20.0), Hit::new(10, 50.0), Hit::new(20, 30.0)]
    );
}

// #[cfg_attr(feature = "flame_it", flame)]
// pub fn intersect_hits(mut and_results: Vec<SearchFieldResult>) -> SearchFieldResult {
//     let mut all_results: FnvHashMap<u32, f32> = FnvHashMap::default();
//     let index_shortest = get_shortest_result(&and_results.iter().map(|el| el.hits_vec.iter()).collect());

//     let shortest_result = and_results.swap_remove(index_shortest).hits;
//     for (k, v) in shortest_result {
//         if and_results.iter().all(|ref x| x.hits.contains_key(&k)) {
//             // if all hits contain this key
//             // all_results.insert(k, v);
//             let score: f32 = and_results.iter().map(|el| *el.hits.get(&k).unwrap_or(&0.0)).sum();
//             all_results.insert(k, v + score);
//         }
//     }
//     // all_results
//     SearchFieldResult {
//         hits: all_results,
//         ..Default::default()
//     }
// }

#[cfg_attr(feature = "flame_it", flame)]
pub fn intersect_hits_vec(mut and_results: Vec<SearchFieldResult>) -> SearchFieldResult {
    if and_results.len() == 1 {
        return and_results.swap_remove(0);
    }
    let index_shortest = get_shortest_result(&and_results.iter().map(|el| el.hits_vec.iter()).collect());

    for res in and_results.iter_mut() {
        res.hits_vec.sort_unstable_by_key(|el| el.id); //TODO ALSO DEDUP???
    }
    let mut shortest_result = and_results.swap_remove(index_shortest).hits_vec;

    // let mut iterators = &and_results.iter().map(|el| el.hits_vec.iter()).collect::<Vec<_>>();

    let mut iterators_and_current = and_results
        .iter_mut()
        .map(|el| {
            let mut iterator = el.hits_vec.iter();
            let current = iterator.next();
            (iterator, current)
        })
        .filter(|el| el.1.is_some())
        .map(|el| (el.0, el.1.unwrap()))
        .collect::<Vec<_>>();

    // shortest_result.retain(|&current_el| {
    //     let current_id = current_el.0;
    //     let current_score = current_el.1;
    //     if iterators_and_current
    //         .iter_mut()
    //         .all(|ref mut iter_n_current| {
    //             if iter_n_current.1 == current_id {
    //                 return true;
    //             }
    //             let iter = &mut iter_n_current.0;
    //             while let Some(el) = iter.next() {
    //                 let id = el.0;
    //                 iter_n_current.1 = id;
    //                 if id > current_id {
    //                     return false;
    //                 }
    //                 if id == current_id {
    //                     return true;
    //                 }
    //             }
    //             return false;
    //         })
    //     {
    //         return true;
    //     }
    //     {
    //         return false;
    //     }
    // });

    let mut intersected_hits = Vec::with_capacity(shortest_result.len());
    for current_el in shortest_result.iter_mut() {
        let current_id = current_el.id;
        let current_score = current_el.score;

        if iterators_and_current.iter_mut().all(|ref mut iter_n_current| {
            // let current_data = &mut iter_n_current.1;
            if (iter_n_current.1).id == current_id {
                return true;
            }
            let iter = &mut iter_n_current.0;
            while let Some(el) = iter.next() {
                let id = el.id;
                iter_n_current.1 = el;
                if id > current_id {
                    return false;
                }
                if id == current_id {
                    return true;
                }
            }
            false
        })
        {
            let mut score = iterators_and_current.iter().map(|el| (el.1).score).sum();
            score += current_score; //TODO SCORE Max oder Sum FOR AND
            intersected_hits.push(Hit::new(current_id, score));
        }
    }
    // all_results
    SearchFieldResult {
        hits_vec: intersected_hits,
        ..Default::default()
    }
}

#[test]
fn intersect_hits_vec_test() {
    let hits1 = vec![Hit::new(10, 20.0), Hit::new(0, 20.0), Hit::new(5, 20.0)]; // unsorted
    let hits2 = vec![Hit::new(0, 20.0), Hit::new(3, 20.0), Hit::new(10, 30.0), Hit::new(20, 30.0)];

    let yop = vec![
        SearchFieldResult {
            hits_vec: hits1,
            ..Default::default()
        },
        SearchFieldResult {
            hits_vec: hits2,
            ..Default::default()
        },
    ];

    let res = intersect_hits_vec(yop);

    assert_eq!(res.hits_vec, vec![Hit::new(0, 40.0), Hit::new(10, 50.0)]);
}


#[cfg_attr(feature = "flame_it", flame)]
pub fn boost_intersect_hits_vec(mut results: SearchFieldResult, mut boost: SearchFieldResult) -> SearchFieldResult {
    results.hits_vec.sort_unstable_by_key(|el| el.id); //TODO SORT NEEDED??
    boost.hits_vec.sort_unstable_by_key(|el| el.id); //TODO SORT NEEDED??

    let mut boost_iter = boost.hits_vec.into_iter();
    apply_boost_from_iter(results, &mut boost_iter) // TODO FIXME

}

fn apply_boost_from_iter(mut results: SearchFieldResult, mut boost_iter: &mut Iterator<Item=Hit>) -> SearchFieldResult {

    let move_boost = |hit:&mut Hit, hit_curr:&mut Hit, boost_iter: &mut Iterator<Item=Hit>|{ //Forward the boost iterator and look for matches
        while let Some(b_hit) = boost_iter.next() {
            if b_hit.id > hit.id {
                *hit_curr = b_hit;
                break;
            }else if b_hit.id == hit.id{
                *hit_curr = b_hit;
                hit.score *= b_hit.score;
                break;
            }
        }
    };

    if let Some(yep) = boost_iter.next(){
        let mut hit_curr = yep;
        for mut hit in results.hits_vec.iter_mut() {
            if hit_curr.id < hit.id {
                move_boost(&mut hit, &mut hit_curr, &mut boost_iter);
            }else if hit_curr.id == hit.id{
                hit.score *= hit_curr.score;
                move_boost(&mut hit, &mut hit_curr, &mut boost_iter); // Possible multi boosts [id:0->2, id:0->4 ...]
            }
        }
    }

    results
}

#[cfg_attr(feature = "flame_it", flame)]
pub fn boost_intersect_hits_vec_multi(mut results: SearchFieldResult, boost: &mut Vec<SearchFieldResult>) -> SearchFieldResult {
    {
        debug_time!("boost hits sort input".to_string());
        results.hits_vec.sort_unstable_by_key(|el| el.id); //TODO SORT NEEDED??
        for res in boost.iter_mut() {
            res.hits_vec.sort_unstable_by_key(|el| el.id);
            res.hits_ids.sort_unstable();
        }
    }
    // let mut boosts =
    let mut boost_iter = boost.iter()
        .map(|el| {
            let boost_val:f32 = el.request.boost.unwrap_or(2.0).clone();
            el.hits_ids.iter().map(move|id| Hit::new(*id, boost_val )) //TODO create version for hits_vec
        })
        .into_iter().kmerge_by(|a, b| a.id < b.id);

    debug_time!("boost_intersect_hits_vec_multi".to_string());
    apply_boost_from_iter(results, &mut boost_iter)
}


#[test]
fn boost_intersect_hits_vec_test_multi() {
    let hits1 = vec![Hit::new(10, 20.0), Hit::new(0, 20.0), Hit::new(5, 20.0), Hit::new(60, 20.0)]; // unsorted
    let boost = vec![0, 3, 10, 70];
    let boost2 = vec![10, 60];

    let mut boosts = vec![SearchFieldResult {hits_ids: boost, ..Default::default() },SearchFieldResult {hits_ids: boost2, ..Default::default() }];

    let res = boost_intersect_hits_vec_multi(SearchFieldResult {hits_vec: hits1, ..Default::default() }, &mut boosts);
    // println!("{:?}", res.hits_vec);

    assert_eq!(res.hits_vec, vec![Hit::new(0, 40.0), Hit::new(5, 20.0), Hit::new(10, 80.0), Hit::new(60, 40.0)]);
}


#[test]
fn boost_intersect_hits_vec_test() {
    let hits1 = vec![Hit::new(10, 20.0), Hit::new(0, 20.0), Hit::new(5, 20.0)]; // unsorted
    let boost = vec![Hit::new(0, 20.0), Hit::new(3, 20.0), Hit::new(10, 30.0), Hit::new(20, 30.0)];


    let res = boost_intersect_hits_vec(SearchFieldResult {hits_vec: hits1, ..Default::default() }, SearchFieldResult {hits_vec: boost, ..Default::default() });
    // println!("{:?}", res.hits_vec);

    assert_eq!(res.hits_vec, vec![Hit::new(0, 400.0), Hit::new(5, 20.0), Hit::new(10, 600.0)]);
}

#[bench]
fn bench_boost_intersect_hits_vec(b: &mut test::Bencher) {
    let hits1:Vec<Hit> = (0..4_000_00).map(|i|Hit::new(i*5 as u32 , 2.2 as f32)).collect();
    let hits2:Vec<Hit> = (0..40_000).map(|i|Hit::new(i*3 as u32, 2.2 as f32)).collect();

    b.iter(|| boost_intersect_hits_vec(SearchFieldResult {hits_vec: hits1.clone(), ..Default::default() }, SearchFieldResult {hits_vec: hits2.clone(), ..Default::default() }))
}

#[bench]
fn bench_boost_intersect_hits_vec_multi(b: &mut test::Bencher) {
    let hits1:Vec<Hit> = (0..4_000_00).map(|i|Hit::new(i*5 as u32 , 2.2 as f32)).collect();
    let hits2:Vec<Hit> = (0..40_000).map(|i|Hit::new(i*3 as u32, 2.2 as f32)).collect();

    b.iter(|| boost_intersect_hits_vec_multi(SearchFieldResult {hits_vec: hits1.clone(), ..Default::default() }, &mut vec![SearchFieldResult {hits_vec: hits2.clone(), ..Default::default() }]))
}

// #[bench]
// fn bench_intersect_hits_vec(b: &mut test::Bencher) {
//     let hits1 = (0..4_000_00).map(|i|(i*5, 2.2)).collect();
//     let hits2 = (0..40_000).map(|i|(i*3, 2.2)).collect();

//     let yop = vec![
//         SearchFieldResult {
//             hits_vec: hits1,
//             ..Default::default()
//         },
//         SearchFieldResult {
//             hits_vec: hits2,
//             ..Default::default()
//         },
//     ];

//     b.iter(|| intersect_hits_vec())
// }

use expression::ScoreExpression;

#[allow(dead_code)]
#[derive(Debug)]
struct BoostIter {
    // iterHashmap: IterMut<K, V> (&'a K, &'a mut V)
}

#[cfg_attr(feature = "flame_it", flame)]
pub fn add_boost(persistence: &Persistence, boost: &RequestBoostPart, hits: &mut SearchFieldResult) -> Result<(), SearchError> {
    // let key = util::boost_path(&boost.path);
    let boost_path = boost.path.to_string() + ".boost_valid_to_value";
    let boostkv_store = persistence.get_boost(&boost_path)?;
    let boost_param = boost.param.unwrap_or(0.0);

    let expre = boost.expression.as_ref().map(|expression| ScoreExpression::new(expression.clone()));
    let default = vec![];
    let skip_when_score = boost.skip_when_score.as_ref().unwrap_or(&default);
    for hit in hits.hits_vec.iter_mut() {
        if skip_when_score.len() > 0 && skip_when_score.iter().find(|x| *x == &hit.score).is_some() {
            continue;
        }
        let value_id = &hit.id;
        let mut score = &mut hit.score;
        // let ref vals_opt = boostkv_store.get(*value_id as usize);
        let ref val_opt = boostkv_store.get_value(*value_id as u64);

        val_opt.as_ref().map(|boost_value| {
            debug!("Found in boosting for value_id {:?}: {:?}", value_id, val_opt);
            let boost_value = *boost_value;
            match boost.boost_fun {
                Some(BoostFunction::Log10) => {
                    let prev_score = *score;
                    *score += (boost_value as f32 + boost_param).log10(); // @Temporary // @Hack // @Cleanup // @FixMe
                    trace!(
                        "boosting value_id {:?} score {:?} with token_value {:?} boost_value {:?} to {:?}",
                        *value_id,
                        prev_score,
                        boost_value,
                        (boost_value as f32 + boost_param).log10(),
                        *score
                    );
                }
                Some(BoostFunction::Linear) => {
                    *score *= boost_value as f32 + boost_param; // @Temporary // @Hack // @Cleanup // @FixMe
                }
                Some(BoostFunction::Add) => {
                    trace!(
                        "boosting value_id {:?} score {:?} with token_value {:?} boost_value {:?} to {:?}",
                        *value_id,
                        score,
                        boost_value,
                        (boost_value as f32 + boost_param),
                        *score + (boost_value as f32 + boost_param)
                    );
                    *score += boost_value as f32 + boost_param;
                }
                None => {}
            }
            expre.as_ref().map(|exp| {
                let prev_score = *score;
                *score += exp.get_score(boost_value as f32);
                trace!(
                    "boost {:?} to {:?} with boost_fun({:?})={:?}",
                    prev_score,
                    score,
                    boost_value,
                    exp.get_score(boost_value as f32)
                );
            });
        });
    }
    Ok(())
}

use fnv;

#[derive(Debug)]
pub enum SearchError {
    Io(io::Error),
    StringError(String),
    MetaData(serde_json::Error),
    Utf8Error(std::str::Utf8Error),
    FstError(fst::Error),
    FstLevenShtein(fst_levenshtein::Error),
    CrossBeamError(crossbeam_channel::SendError<std::collections::HashMap<u32, f32, std::hash::BuildHasherDefault<fnv::FnvHasher>>>),
    CrossBeamError2(crossbeam_channel::SendError<SearchFieldResult>),
    CrossBeamErrorReceive(crossbeam_channel::RecvError),
    TooManyStates,
}
// Automatic Conversion
impl From<io::Error> for SearchError {
    fn from(err: io::Error) -> SearchError {
        SearchError::Io(err)
    }
}
impl From<serde_json::Error> for SearchError {
    fn from(err: serde_json::Error) -> SearchError {
        SearchError::MetaData(err)
    }
}
impl From<std::str::Utf8Error> for SearchError {
    fn from(err: std::str::Utf8Error) -> SearchError {
        SearchError::Utf8Error(err)
    }
}
impl From<fst::Error> for SearchError {
    fn from(err: fst::Error) -> SearchError {
        SearchError::FstError(err)
    }
}
impl From<fst_levenshtein::Error> for SearchError {
    fn from(err: fst_levenshtein::Error) -> SearchError {
        SearchError::FstLevenShtein(err)
    }
}
impl From<crossbeam_channel::SendError<std::collections::HashMap<u32, f32, std::hash::BuildHasherDefault<fnv::FnvHasher>>>> for SearchError {
    fn from(err: crossbeam_channel::SendError<std::collections::HashMap<u32, f32, std::hash::BuildHasherDefault<fnv::FnvHasher>>>) -> SearchError {
        SearchError::CrossBeamError(err)
    }
}
impl From<crossbeam_channel::SendError<SearchFieldResult>> for SearchError {
    fn from(err: crossbeam_channel::SendError<SearchFieldResult>) -> SearchError {
        SearchError::CrossBeamError2(err)
    }
}
impl From<crossbeam_channel::RecvError> for SearchError {
    fn from(err: crossbeam_channel::RecvError) -> SearchError {
        SearchError::CrossBeamErrorReceive(err)
    }
}

impl From<String> for SearchError {
    fn from(err: String) -> SearchError {
        SearchError::StringError(err)
    }
}

impl<'a> From<&'a str> for SearchError {
    fn from(err: &'a str) -> SearchError {
        SearchError::StringError(err.to_string())
    }
}

use std::fmt;
pub use std::error::Error;

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "\n{}", self)?;
        Ok(())
    }
}

impl Error for SearchError {
    fn description(&self) -> &str {
        "self.error.description()"
    }

    fn cause(&self) -> Option<&Error> {
        None
    }
}

// use util::*;

// pub fn read_data_single(persistence: &Persistence, id: u32, field: String) -> Result<String, SearchError> {
//     let steps = util::get_steps_to_anchor(&field);

//     let mut data = vec![id];
//     let mut result = json!({});

//     for path in steps.iter() {
//         result[path.clone()] = json!([]);
//         let dat:FnvHashMap<u32, Vec<u32>> = join_for_read(persistence, data, &concat(path, ".parentToValueId"))?;
//         data = dat.get(&id).ok_or(From::from(format!("Could not find id {:?} in  {:?} {:?}", id, path, dat)))?.clone();
//     }

//     let texto = get_id_text_map_for_ids(persistence, steps.last().unwrap(), &data);
//     println!("{:?}", texto);
//     Ok(serde_json::to_string_pretty(&result).unwrap())
//     // "".to_string()
// }

#[cfg_attr(feature = "flame_it", flame)]
pub fn read_tree(persistence: &Persistence, id: u32, tree: &NodeTree) -> Result<serde_json::Value, SearchError> {
    let mut json = json!({});

    for (prop, sub_tree) in tree.next.iter() {
        if sub_tree.is_leaf {
            let text_value_id_opt = join_for_1_to_1(persistence, id, &concat(&prop, ".parentToValueId"))?;
            if let Some(text_value_id) = text_value_id_opt {
                let texto = get_text_for_id(persistence, &prop, text_value_id);
                json[extract_prop_name(prop)] = json!(texto);
            }
        } else if let Some(sub_ids) = join_for_1_to_n(persistence, id, &concat(&prop, ".parentToValueId"))? {
            let is_flat = sub_tree.next.len() == 1 && sub_tree.next.keys().nth(0).unwrap().ends_with("[].textindex");
            if is_flat {
                let flat_prop = sub_tree.next.keys().nth(0).unwrap();
                //text_id for value_ids
                let text_ids: Vec<u32> = sub_ids
                    .iter()
                    .flat_map(|id| join_for_1_to_1(persistence, *id, &concat(&flat_prop, ".parentToValueId")).unwrap())
                    .collect();
                let texto = get_text_for_ids(persistence, flat_prop, &text_ids);
                json[extract_prop_name(prop)] = json!(texto);
            } else {
                let is_array = prop.ends_with("[]");
                if is_array {
                    let mut sub_data = vec![];
                    for sub_id in sub_ids {
                        sub_data.push(read_tree(persistence, sub_id, &sub_tree)?);
                    }
                    json[extract_prop_name(prop)] = json!(sub_data);
                } else if let Some(sub_id) = sub_ids.get(0) {
                    // println!("KEIN ARRAY {:?}", sub_tree.clone());
                    json[extract_prop_name(prop)] = read_tree(persistence, *sub_id, &sub_tree)?;
                }
            }
        }
    }
    Ok(json)
}

//TODO CHECK FIELD VALIDTY
pub fn get_read_tree_from_fields(_persistence: &Persistence, fields: &[String]) -> util::NodeTree {
    let all_steps: Vec<Vec<String>> = fields.iter().map(|field| util::get_steps_to_anchor(&field)).collect();
    println!("{:?}", all_steps);
    to_node_tree(all_steps)
}

pub fn read_data(persistence: &Persistence, id: u32, fields: &[String]) -> Result<serde_json::Value, SearchError> {
    // let all_steps: FnvHashMap<String, Vec<String>> = fields.iter().map(|field| (field.clone(), util::get_steps_to_anchor(&field))).collect();
    // let all_steps: Vec<Vec<String>> = fields.iter().map(|field| util::get_steps_to_anchor(&field)).collect();
    // let paths = util::get_steps_to_anchor(&request.path);
    // let tree = to_node_tree(all_steps);
    let tree = get_read_tree_from_fields(persistence, fields);
    read_tree(persistence, id, &tree)
    // Ok(serde_json::to_string_pretty(&dat).unwrap())
}

#[cfg_attr(feature = "flame_it", flame)]
pub fn join_to_parent_with_score(
    persistence: &Persistence,
    input: SearchFieldResult,
    path: &str,
    trace_time_info: &str,
) -> Result<SearchFieldResult, SearchError> {
    let mut total_values = 0;
    let mut hits: FnvHashMap<u32, f32> = FnvHashMap::default();
    let hits_iter = input.hits_vec.into_iter();
    let num_hits = hits_iter.size_hint().1.unwrap_or(0);
    hits.reserve(num_hits);
    let kv_store = persistence.get_valueid_to_parent(path)?;
    // debug_time!("term hits hit to column");
    debug_time!(format!("{:?} {:?}", path, trace_time_info));
    for hit in hits_iter {
        let term_id = hit.id;
        let mut score = hit.score;
        let ref values = kv_store.get_values(term_id as u64);
        values.as_ref().map(|values| {
            total_values += values.len();
            hits.reserve(values.len());
            // trace!("value_id: {:?} values: {:?} ", value_id, values);
            for parent_val_id in values {
                // @Temporary
                match hits.entry(*parent_val_id as u32) {
                    Vacant(entry) => {
                        trace!("value_id: {:?} to parent: {:?} score {:?}", term_id, parent_val_id, score);
                        entry.insert(score);
                    }
                    Occupied(entry) => {
                        if *entry.get() < score {
                            trace!("value_id: {:?} to parent: {:?} score: {:?}", term_id, parent_val_id, score.max(*entry.get()));
                            *entry.into_mut() = score.max(*entry.get());
                        }
                    }
                }
            }
        });
    }
    debug!(
        "{:?} hits hit {:?} distinct ({:?} total ) in column {:?}",
        num_hits,
        hits.len(),
        total_values,
        path
    );

    // debug!("{:?} hits in next_level_hits {:?}", next_level_hits.len(), &concat(path_name, ".valueIdToParent"));

    // trace!("next_level_hits from {:?}: {:?}", &concat(path_name, ".valueIdToParent"), hits);
    // debug!("{:?} hits in next_level_hits {:?}", hits.len(), &concat(path_name, ".valueIdToParent"));

    Ok(SearchFieldResult {
        hits_vec: hits.into_iter().map(|(k, v)| Hit::new(k, v)).collect(),
        ..Default::default()
    })
}

#[cfg_attr(feature = "flame_it", flame)]
pub fn join_for_read(persistence: &Persistence, input: Vec<u32>, path: &str) -> Result<FnvHashMap<u32, Vec<u32>>, SearchError> {
    let mut hits: FnvHashMap<u32, Vec<u32>> = FnvHashMap::default();
    let kv_store = persistence.get_valueid_to_parent(path)?;
    // debug_time!("term hits hit to column");
    debug_time!(format!("{:?} ", path));
    for value_id in input {
        let ref values = kv_store.get_values(value_id as u64);
        values.as_ref().map(|values| {
            hits.reserve(values.len());
            hits.insert(value_id, values.clone());
        });
    }
    debug!("hits hit {:?} distinct in column {:?}", hits.len(), path);

    Ok(hits)
}
#[cfg_attr(feature = "flame_it", flame)]
pub fn join_for_1_to_1(persistence: &Persistence, value_id: u32, path: &str) -> Result<std::option::Option<u32>, SearchError> {
    let kv_store = persistence.get_valueid_to_parent(path)?;
    Ok(kv_store.get_value(value_id as u64))
}
#[cfg_attr(feature = "flame_it", flame)]
pub fn join_for_1_to_n(persistence: &Persistence, value_id: u32, path: &str) -> Result<Option<Vec<u32>>, SearchError> {
    let kv_store = persistence.get_valueid_to_parent(path)?;
    Ok(kv_store.get_values(value_id as u64))
}

// #[cfg_attr(feature="flame_it", flame)]
// fn join_to_parent<I>(persistence: &Persistence, input: I, path: &str, trace_time_info: &str) -> FnvHashMap<u32, f32>
//     where
//     I: IntoIterator<Item = (u32, f32)> ,
// {
//     let mut total_values = 0;
//     let mut hits: FnvHashMap<u32, f32> = FnvHashMap::default();
//     let hits_iter = input.into_iter();
//     let num_hits = hits_iter.size_hint().1.unwrap_or(0);
//     hits.reserve(num_hits);
//     let kv_store = persistence.get_valueid_to_parent(&concat(&path, ".valueIdToParent"));
//     // debug_time!("term hits hit to column");
//     debug_time!(format!("{:?} {:?}", path, trace_time_info));
//     for (term_id, score) in hits_iter {
//         let ref values = kv_store.get_values(term_id as u64);
//         values.as_ref().map(|values| {
//             total_values += values.len();
//             hits.reserve(values.len());
//             // trace!("value_id: {:?} values: {:?} ", value_id, values);
//             for parent_val_id in values {
//                 // @Temporary
//                 match hits.entry(*parent_val_id as u32) {
//                     Vacant(entry) => {
//                         trace!("value_id: {:?} to parent: {:?} score {:?}", term_id, parent_val_id, score);
//                         entry.insert(score);
//                     }
//                     Occupied(entry) => if *entry.get() < score {
//                         trace!("value_id: {:?} to parent: {:?} score: {:?}", term_id, parent_val_id, score.max(*entry.get()));
//                         *entry.into_mut() = score.max(*entry.get());
//                     },
//                 }
//             }
//         });
//     }
//     debug!("{:?} hits hit {:?} distinct ({:?} total ) in column {:?}", num_hits, hits.len(), total_values, path);

//     // debug!("{:?} hits in next_level_hits {:?}", next_level_hits.len(), &concat(path_name, ".valueIdToParent"));

//     // trace!("next_level_hits from {:?}: {:?}", &concat(path_name, ".valueIdToParent"), hits);
//     // debug!("{:?} hits in next_level_hits {:?}", hits.len(), &concat(path_name, ".valueIdToParent"));

//     hits
// }

// #[cfg_attr(feature="flame_it", flame)]
// pub fn search_raw(
//     persistence: &Persistence, mut request: RequestSearchPart, boost: Option<Vec<RequestBoostPart>>
// ) -> Result<FnvHashMap<u32, f32>, SearchError> {
//     // request.term = util::normalize_text(&request.term);
//     request.terms = request.terms.iter().map(|el| util::normalize_text(el)).collect::<Vec<_>>();
//     debug_time!("search and join to anchor");

//     let step = plan_creator_search_part(request.clone(), boost);

//     let yep = step.get_output();

//     execute_step(step, persistence)?;
//     let hits = yep.recv().unwrap();
//     Ok(hits)
// }

// pub fn test_levenshtein(term:&str, max_distance:u32) -> Result<(Vec<String>), io::Error> {

//     use std::time::SystemTime;

//     let mut f = try!(File::open("de_full_2.txt"));
//     let mut s = String::new();
//     try!(f.read_to_string(&mut s));

//     let now = SystemTime::now();

//     let lines = s.lines();
//     let mut hits = vec![];
//     for line in lines{
//         let distance = distance(term, line);
//         if distance < max_distance {
//             hits.push(line.to_string())
//         }
//     }

//     let ms = match now.elapsed() {
//         Ok(elapsed) => {(elapsed.as_secs() as f64) * 1_000.0 + (elapsed.subsec_nanos() as f64 / 1000_000.0)}
//         Err(_e) => {-1.0}
//     };

//     let lines_checked = s.lines().count() as f64;
//     println!("levenshtein ms: {}", ms);
//     println!("Lines : {}", lines_checked );
//     let ms_per_1000 = ((ms as f64) / lines_checked) * 1000.0;
//     println!("ms per 1000 lookups: {}", ms_per_1000);
//     Ok((hits))

// }
