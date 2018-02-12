use util::{self, concat};
use fnv::FnvHashMap;
use fnv::FnvHashSet;

use serde_json::{self, Value};

use json_converter;

use std::{self, str};
use std::io;

use persistence::{LoadingType, Persistence};

use util::*;

use log;

#[allow(unused_imports)]
use fst::{self, IntoStreamer, MapBuilder, Set};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum CreateIndex {
    FulltextInfo(Fulltext),
    BoostInfo(Boost),
    FacetInfo(FacetIndex),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FacetIndex {
    facet: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Fulltext {
    fulltext: String,
    options: Option<FulltextIndexOptions>,
    loading_type: Option<LoadingType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Boost {
    boost: String,
    options: BoostIndexOptions,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TokenValuesConfig {
    path: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct FulltextIndexOptions {
    pub tokenize: bool,
    pub add_normal_values: Option<bool>,
    pub stopwords: Option<FnvHashSet<String>>,
}

impl FulltextIndexOptions {
    fn new_with_tokenize() -> FulltextIndexOptions {
        FulltextIndexOptions {
            tokenize: true,
            stopwords: None,
            add_normal_values: Some(true),
        }
    }
    #[allow(dead_code)]
    fn new_without_tokenize() -> FulltextIndexOptions {
        FulltextIndexOptions {
            tokenize: true,
            stopwords: None,
            add_normal_values: Some(true),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BoostIndexOptions {
    boost_type: String, // type:
}

#[derive(Debug, Default)]
pub struct TermInfo {
    pub id: u32,
    pub num_occurences: u32,
}

impl TermInfo {
    pub fn new(id: u32) -> TermInfo {
        TermInfo { id: id, num_occurences: 0 }
    }
}

pub fn set_ids(terms: &mut FnvHashMap<String, TermInfo>) {
    let mut v: Vec<String> = terms
        .keys()
        // .collect::<Vec<&String>>()
        // .iter()
        .map(|el| (*el).clone())
        .collect();
    v.sort();
    for (i, term) in v.iter().enumerate() {
        // terms.get_mut(term)
        if let Some(term_info) = terms.get_mut(&term.clone()) {
            term_info.id = i as u32;
        }
    }
}

pub trait GetValueId {
    fn get_value_id(&self) -> u32;
}

#[derive(Debug, Default, Clone)]
pub struct ValIdPair {
    pub valid: u32,
    pub parent_val_id: u32,
}

#[derive(Debug, Default, Clone)]
pub struct ValIdPairToken {
    pub valid: u32,
    pub parent_val_id: u32,
    pub token_pos: u32,
    pub num_occurences: u32,
    pub entry_num_tokens: u32,
}

#[derive(Debug, Default, Clone)]
pub struct TokenToAnchorScore {
    pub valid: u32,
    pub anchor_id: u32,
    pub score: u32,
}

impl GetValueId for ValIdPair {
    fn get_value_id(&self) -> u32 {
        self.valid
    }
}
impl GetValueId for ValIdPairToken {
    fn get_value_id(&self) -> u32 {
        self.valid
    }
}

/// Used for boost
/// e.g. boost value 5000 for id 5
/// 5 -> 5000
#[derive(Debug, Clone)]
pub struct ValIdToValue {
    pub valid: u32,
    pub value: u32,
}

impl GetValueId for ValIdToValue {
    fn get_value_id(&self) -> u32 {
        self.valid
    }
}

impl std::fmt::Display for ValIdPair {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "\n{}\t{}", self.valid, self.parent_val_id)?;
        Ok(())
    }
}

// use std::fmt;
// use std::fmt::{Display, Formatter, Error};

// impl<ValIdPair> fmt::Display for Vec<ValIdPair> {
//     fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
//         write!(f, "(a, b)",)
//         Ok(())
//     }
// }

#[allow(dead_code)]
fn print_vec(vec: &Vec<ValIdPair>, valid_header: &str, parentid_header: &str) -> String {
    format!("{}\t{}", valid_header, parentid_header)
        + &vec.iter()
            .map(|el| format!("\n{}\t{}", el.valid, el.parent_val_id))
            .collect::<Vec<_>>()
            .join("")
}

use persistence;

fn store_full_text_info(
    persistence: &mut Persistence,
    all_terms: FnvHashMap<String, TermInfo>,
    path: &str,
    options: &FulltextIndexOptions,
) -> Result<(), io::Error> {
    info_time!(format!("store_fst strings and string offsets {:?}", path));
    let mut sorted_terms: Vec<&String> = all_terms.keys().collect::<Vec<&String>>();
    sorted_terms.sort();

    persistence.write_data(
        path,
        sorted_terms
            .iter()
            .fold(String::with_capacity(sorted_terms.len() * 10), |acc, line| acc + line + "\n")
            .as_bytes(),
    )?;
    let offsets = get_string_offsets(&sorted_terms);

    //TEST FST AS ID MAPPER
    // let mut offsets_fst: FnvHashMap<String, TermInfo> = FnvHashMap::default();
    // for (i, offset) in offsets.iter().enumerate() {
    //     let padding = 1;
    //     offsets_fst.insert(
    //         format!("{:0padding$}", i, padding = padding),
    //         TermInfo::new(*offset as u32),
    //     );
    // }
    // store_fst(persistence, &offsets_fst, &concat(&path, ".offsets")).expect("Could not store fst");
    //TEST FST AS ID MAPPER

    persistence.write_index(&persistence::vec_to_bytes_u64(&offsets), &offsets, &concat(&path, ".offsets"))?; // String byte offsets
                                                                                                              // persistence.write_index(&all_terms.iter().map(|ref el| el.len() as u32).collect::<Vec<_>>(), &concat(path, ".length"))?;
    store_fst(persistence, &all_terms, sorted_terms, path).expect("Could not store fst"); // @FixMe handle result
                                                                                          // create_char_offsets(&all_terms, &concat(&path, ""), &mut persistence)?;
    persistence.meta_data.fulltext_indices.insert(path.to_string(), options.clone());
    Ok(())
}

fn store_fst(persistence: &mut Persistence, all_terms: &FnvHashMap<String, TermInfo>, sorted_terms: Vec<&String>, path: &str) -> Result<(), fst::Error> {
    debug_time!(format!("store_fst {:?}", path));
    let wtr = persistence.get_buffered_writer(&concat(&path, ".fst"))?;
    // Create a builder that can be used to insert new key-value pairs.
    let mut build = MapBuilder::new(wtr)?;

    // let mut v: Vec<&String> = all_terms.keys().collect::<Vec<&String>>();
    // v.sort();
    for term in sorted_terms {
        let term_info = all_terms.get(term).expect("wtf");
        build.insert(term, term_info.id as u64).expect("could not insert into fst");
    }
    // for (term, term_info) in all_terms.iter() {
    //     build.insert(term, term_info.id as u64).unwrap();
    // }
    // Finish construction of the map and flush its contents to disk.
    build.finish()?;

    Ok(())
}
#[allow(unused_imports)]
use sled;
#[allow(unused_imports)]
use byteorder::{LittleEndian, WriteBytesExt};

use tokenizer::*;

fn add_count_text(terms: &mut FnvHashMap<String, TermInfo>, text: &str) {
    if !terms.contains_key(text) {
        terms.insert(text.to_string(), TermInfo::default());
    }
    let stat = terms.get_mut(text).unwrap();
    stat.num_occurences += 1;
}

fn add_text<T: Tokenizer>(text: &str, terms: &mut FnvHashMap<String, TermInfo>, options: &FulltextIndexOptions, tokenizer: &T) {
    trace!("text: {:?}", text);
    if options.stopwords.as_ref().map(|el| el.contains(text)).unwrap_or(false) {
        return;
    }

    add_count_text(terms, text);

    //Add lowercase version for search
    // {
    //     let stat = terms.entry(text.to_lowercase().trim().to_string()).or_insert(TermInfo::default());
    //     stat.num_occurences += 1;
    // }

    if options.tokenize && tokenizer.has_tokens(&text) {
        tokenizer.get_tokens(&text, &mut |token: &str, _is_seperator: bool| {
            // let token_str = token.to_string();
            if options.stopwords.as_ref().map(|el| el.contains(token)).unwrap_or(false) {
                return;
            }
            add_count_text(terms, token);

            // //Add lowercase version for non seperators
            // if !is_seperator{
            //     let stat = terms.entry(token_str.to_lowercase().trim().to_string()).or_insert(TermInfo::default());
            //     stat.num_occurences += 1;
            // }
        });
    }
}

fn get_or_insert<'a, T, F>(map: &'a mut FnvHashMap<String, T>, key: &str, constructor: &F) -> &'a mut T
where
    F: Fn() -> T,
{
    if !map.contains_key(key) {
        map.insert(
            key.to_string(),
            constructor(),
            // FnvHashMap::with_capacity_and_hasher(num_elements, Default::default()),
        );
    }
    map.get_mut(key).unwrap()
}
use itertools::Itertools;
fn calculate_token_score_in_doc(tokens_to_anchor: &mut Vec<ValIdPairToken>) -> Vec<TokenToAnchorScore> {
    // TokenToAnchorScore {
    //     pub valid: u32,
    //     pub anchor_id: u32,
    //     pub score: u32
    // }

    // valid: u32,
    // parent_val_id: u32,
    // token_pos: u32,
    // num_occurences: u32,
    tokens_to_anchor.sort_unstable_by(|a, b| {
        let sort_anch = a.parent_val_id.cmp(&b.parent_val_id);
        if sort_anch == std::cmp::Ordering::Equal {
            let sort_valid = a.valid.cmp(&b.valid);
            if sort_valid == std::cmp::Ordering::Equal {
                a.token_pos.cmp(&b.token_pos)
            } else {
                sort_valid
            }
        } else {
            sort_anch
        }
    }); // sort by parent id

    let mut dat = vec![];
    for (_, mut group) in &tokens_to_anchor.into_iter().group_by(|el| (el.parent_val_id, el.valid)) {
        let first = group.next().unwrap();
        let best_pos = first.token_pos;

        let mut avg_pos = best_pos;
        let mut num_occurences_in_doc = 1;

        let mut exact_match_boost = 1;
        if first.entry_num_tokens == 1 && first.token_pos == 0 {
            exact_match_boost = 2
        }

        for el in group {
            avg_pos = avg_pos + (el.token_pos - avg_pos) / num_occurences_in_doc;
            num_occurences_in_doc += 1;
        }

        // let mut score = ((20 / (best_pos + 2)) + num_occurences_in_doc.log10() ) / first.num_occurences;
        let mut score = 20 / (best_pos + 10);
        score *= exact_match_boost;

        // println!("best_pos {:?}",best_pos);
        // println!("num_occurences_in_doc {:?}",num_occurences_in_doc);
        // println!("first.num_occurences {:?}",first.num_occurences);
        // println!("scorescore {:?}",score);

        dat.push(TokenToAnchorScore {
            valid: first.valid,
            anchor_id: first.parent_val_id,
            score: score,
        });
    }

    dat
}

pub fn get_allterms(data: &Value, fulltext_info_for_path: &FnvHashMap<String, Fulltext>) -> FnvHashMap<String, FnvHashMap<String, TermInfo>> {
    info_time!("get_allterms dictionary");
    let mut terms_in_path: FnvHashMap<String, FnvHashMap<String, TermInfo>> = FnvHashMap::default();

    let mut opt = json_converter::ForEachOpt {};
    let mut id_holder = json_converter::IDHolder::new();

    let num_elements = if let Some(arr) = data.as_array() { arr.len() } else { 1 };

    let tokenizer = SimpleTokenizerCharsIterateGroupTokens {};
    let default_fulltext_options = FulltextIndexOptions::new_with_tokenize();

    {
        let mut cb_text = |_anchor_id: u32, value: &str, path: &str, _parent_val_id: u32| {
            let options: &FulltextIndexOptions = fulltext_info_for_path
                .get(path)
                .and_then(|el| el.options.as_ref())
                .unwrap_or(&default_fulltext_options);

            let mut terms = get_or_insert(&mut terms_in_path, path, &|| {
                FnvHashMap::with_capacity_and_hasher(num_elements, Default::default())
            });

            add_text(value, &mut terms, &options, &tokenizer);
        };

        let mut callback_ids = |_anchor_id: u32, _path: &str, _value_id: u32, _parent_val_id: u32| {};

        json_converter::for_each_element(&data, &mut id_holder, &mut opt, &mut cb_text, &mut callback_ids);
    }

    {
        for mut terms in terms_in_path.values_mut() {
            set_ids(&mut terms);
        }
    }
    terms_in_path
}

#[derive(Debug, Default, Clone)]
struct PathData {
    tokens_to_parent: Vec<ValIdPair>,
    tokens_to_anchor: Vec<ValIdPairToken>,
    value_id_to_token_ids: Vec<ValIdPair>,
    text_id_to_parent: Vec<ValIdPair>,
    anchor_to_text_id: Option<Vec<ValIdPair>>,
    boost: Option<Vec<ValIdToValue>>,
}

pub fn create_fulltext_index(data: &Value, mut persistence: &mut Persistence, indices_json: &Vec<CreateIndex>) -> Result<(), io::Error> {
    // let data: Value = serde_json::from_str(data_str).unwrap();
    let num_elements = if let Some(arr) = data.as_array() { arr.len() } else { 1 };

    let fulltext_info_for_path: FnvHashMap<String, Fulltext> = indices_json
        .iter()
        .flat_map(|index| match index {
            &CreateIndex::FulltextInfo(ref el) => Some(el),
            &CreateIndex::BoostInfo(_) => None,
            &CreateIndex::FacetInfo(_) => None,
        })
        .map(|fulltext_info| (fulltext_info.fulltext.to_string() + ".textindex", (*fulltext_info).clone()))
        .collect();

    let boost_info_for_path: FnvHashMap<String, Boost> = indices_json
        .iter()
        .flat_map(|index| match index {
            &CreateIndex::FulltextInfo(_) => None,
            &CreateIndex::BoostInfo(ref el) => Some(el),
            &CreateIndex::FacetInfo(_) => None,
        })
        .map(|boost_info| (boost_info.boost.to_string() + ".textindex", (*boost_info).clone()))
        .collect();

    let facet_index: FnvHashSet<String> = indices_json
        .iter()
        .flat_map(|index| match index {
            &CreateIndex::FulltextInfo(_) => None,
            &CreateIndex::BoostInfo(_) => None,
            &CreateIndex::FacetInfo(ref el) => Some(el.facet.to_string() + ".textindex"),
        })
        .collect();

    let all_terms_in_path = get_allterms(&data, &fulltext_info_for_path);
    info_time!("create_fulltext_index");
    trace!("all_terms {:?}", all_terms_in_path);

    let mut opt = json_converter::ForEachOpt {};
    let mut id_holder = json_converter::IDHolder::new();

    let mut path_data: FnvHashMap<String, PathData> = FnvHashMap::default();

    let mut tuples_to_parent_in_path: FnvHashMap<String, Vec<ValIdPair>> = FnvHashMap::default();

    let default_fulltext_options = FulltextIndexOptions::new_with_tokenize();

    let tokenizer = SimpleTokenizerCharsIterateGroupTokens {};
    {
        info_time!(format!("extract text and ids"));
        let mut cb_text = |anchor_id: u32, value: &str, path: &str, parent_val_id: u32| {
            let data = get_or_insert(&mut path_data, path, &|| {
                let boost_info_data = if boost_info_for_path.contains_key(path) { Some(vec![]) } else { None };

                let anchor_to_text_id = if facet_index.contains(path) { Some(vec![]) } else { None };
                PathData {
                    anchor_to_text_id: anchor_to_text_id,
                    boost: boost_info_data,
                    ..Default::default()
                }
            });

            let all_terms = all_terms_in_path.get(path).unwrap();

            let options: &FulltextIndexOptions = fulltext_info_for_path
                .get(path)
                .and_then(|el| el.options.as_ref())
                .unwrap_or(&default_fulltext_options);

            if options.stopwords.as_ref().map(|el| el.contains(value)).unwrap_or(false) {
                return;
            }

            let text_info = all_terms.get(value).expect("did not found term");

            data.text_id_to_parent.push(ValIdPair {
                valid: text_info.id as u32,
                parent_val_id: parent_val_id,
            });
            data.anchor_to_text_id.as_mut().map(|el| {
                el.push(ValIdPair {
                    valid: anchor_id,
                    parent_val_id: text_info.id as u32,
                })
            });
            data.boost.as_mut().map(|el| {
                // if options.boost_type == "int" {
                let my_int = value.parse::<u32>().expect(&format!("Expected an int value but got {:?}", value));
                el.push(ValIdToValue {
                    valid: parent_val_id,
                    value: my_int,
                });
                // } // TODO More cases
            });
            trace!("Found id {:?} for {:?}", text_info, value);

            data.tokens_to_anchor.push(ValIdPairToken {
                valid: text_info.id as u32,
                num_occurences: text_info.num_occurences as u32,
                parent_val_id: anchor_id as u32,
                token_pos: 0,
                entry_num_tokens: 1,
            });

            if options.tokenize && tokenizer.has_tokens(value) {
                let mut tokens_to_anchor = vec![];
                let mut token_pos = 0;
                tokenizer.get_tokens(value, &mut |token: &str, _is_seperator: bool| {
                    if options.stopwords.as_ref().map(|el| el.contains(token)).unwrap_or(false) {
                        return;
                    }

                    let token_info = all_terms.get(token).expect("did not found token");
                    trace!("Adding to tokens {:?} : {:?}", token, token_info);

                    data.value_id_to_token_ids.push(ValIdPair {
                        valid: text_info.id as u32,
                        parent_val_id: token_info.id as u32,
                    });
                    data.tokens_to_parent.push(ValIdPair {
                        valid: token_info.id as u32,
                        parent_val_id: text_info.id as u32,
                    });
                    tokens_to_anchor.push(ValIdPairToken {
                        valid: token_info.id as u32,
                        num_occurences: token_info.num_occurences as u32,
                        parent_val_id: anchor_id as u32,
                        token_pos: token_pos as u32,
                        entry_num_tokens: 0,
                    });
                    token_pos += 1;
                });

                for mut el in tokens_to_anchor {
                    el.entry_num_tokens = token_pos;
                    data.tokens_to_anchor.push(el);
                }
            }
        };

        let mut callback_ids = |_anchor_id: u32, path: &str, value_id: u32, parent_val_id: u32| {
            let tuples = get_or_insert(&mut tuples_to_parent_in_path, path, &|| Vec::with_capacity(num_elements));

            tuples.push(ValIdPair {
                valid: value_id,
                parent_val_id: parent_val_id,
            });
        };

        json_converter::for_each_element(&data, &mut id_holder, &mut opt, &mut cb_text, &mut callback_ids);
    }

    let is_sublevel = |path: &str| {
        return path.contains("[]");
    };
    let is_text_id_to_parent = |path: &str| {
        return path.ends_with(".textindex");
    };

    {
        let write_tuples = |persistence: &mut Persistence, path: &str, tuples: &mut Vec<ValIdPair>| -> Result<(), io::Error> {
            let is_alway_1_to_1 = !is_text_id_to_parent(path); // valueIdToParent relation is always 1 to 1, expect for text_ids, which can have multiple parents

            persistence.write_tuple_pair(tuples, &concat(&path, ".valueIdToParent"), is_alway_1_to_1)?;
            if log_enabled!(log::Level::Trace) {
                trace!("{}\n{}", &concat(&path, ".valueIdToParent"), print_vec(&tuples, &path, "parentid"));
            }

            //Flip values
            for el in tuples.iter_mut() {
                std::mem::swap(&mut el.parent_val_id, &mut el.valid);
            }
            persistence.write_tuple_pair(tuples, &concat(&path, ".parentToValueId"), !is_sublevel(path))?;
            if log_enabled!(log::Level::Trace) {
                trace!("{}\n{}", &concat(&path, ".parentToValueId"), print_vec(&tuples, &path, "value_id"));
            }
            Ok(())
        };

        for (path, mut data) in path_data {
            persistence.write_tuple_pair_dedup(&mut data.tokens_to_parent, &concat(&path, ".tokens"), true, false)?;
            trace!("{}\n{}", &concat(&path, ".tokens"), print_vec(&data.tokens_to_parent, "token_id", "parent_id"));

            let token_to_anchor_score = calculate_token_score_in_doc(&mut data.tokens_to_anchor);
            let mut token_to_anchor_score_pairs: Vec<ValIdPair> = token_to_anchor_score
                .iter()
                .flat_map(|el| {
                    vec![
                        ValIdPair {
                            valid: el.valid as u32,
                            parent_val_id: el.anchor_id as u32,
                        },
                        ValIdPair {
                            valid: el.valid as u32,
                            parent_val_id: el.score as u32,
                        },
                    ]
                })
                .collect();

            persistence.write_tuple_pair(&mut token_to_anchor_score_pairs, &concat(&path, ".tokens.to_anchor"), false)?;
            trace!(
                "{}\n{}",
                &concat(&path, ".tokens.to_anchor"),
                print_vec(&token_to_anchor_score_pairs, "token_id", "anchor_id")
            );

            // persistence.write_tuple_pair_dedup(&mut data.tokens_to_anchor, &concat(&path, ".tokens.to_anchor"), true)?;
            persistence.write_tuple_pair(&mut data.value_id_to_token_ids, &concat(&path, ".value_id_to_token_ids"), false)?;
            trace!(
                "{}\n{}",
                &concat(&path, ".value_id_to_token_ids"),
                print_vec(&data.value_id_to_token_ids, "value_id", "token_id")
            );

            write_tuples(&mut persistence, &path, &mut data.text_id_to_parent)?;

            if let Some(ref mut anchor_to_text_id) = data.anchor_to_text_id {
                persistence.write_tuple_pair(anchor_to_text_id, &concat(&path, ".anchor_to_text_id"), false)?; // TODO THIS SHOULD ONLY BE USED IN 1 TO N RELATIONS -FIXME PLZZZZ
            }
            if let Some(ref mut tuples) = data.boost {
                persistence.write_boost_tuple_pair(tuples, &extract_field_name(&path))?; // TODO use .textindex in boost?
                                                                                         // persistence.write_tuple_pair(anchor_to_text_id, &concat(&path, ".anchor_to_text_id"))?;
            }
        }

        for (path, all_terms) in all_terms_in_path {
            let options: &FulltextIndexOptions = fulltext_info_for_path
                .get(&path)
                .and_then(|el| el.options.as_ref())
                .unwrap_or(&default_fulltext_options);
            store_full_text_info(&mut persistence, all_terms, &path, &options)?;
        }

        for (path, mut tuples) in tuples_to_parent_in_path.iter_mut() {
            write_tuples(&mut persistence, path, &mut tuples)?;
        }
    }

    // let path_name = util::get_file_path_name(&paths[i], is_text_index);
    // persistence.write_tuple_pair(&mut tuples, &concat(&path_name, ".valueIdToParent"))?;

    //TEST FST AS ID MAPPER
    // let mut all_ids_as_str: FnvHashMap<String, TermInfo> = FnvHashMap::default();
    // for pair in &tuples {
    //     let padding = 10;
    //     all_ids_as_str.insert(format!("{:0padding$}", pair.valid, padding = padding), TermInfo::new(pair.parent_val_id)); // COMPRESSION 50-90%
    // }
    // store_fst(persistence, &all_ids_as_str, &concat(&path_name, ".valueIdToParent.fst")).expect("Could not store fst");
    //TEST FST AS ID MAPPER

    // if is_text_index && options.tokenize {
    //     persistence.write_tuple_pair(&mut tokens, &concat(&path_name, ".tokens"))?;
    //     trace!("{}\n{}",&concat(&path_name, ".tokens"), print_vec(&tokens, &concat(&path_name, ".tokenid"), &concat(&path_name, ".valueid")));
    // }

    // store_full_text_info(&mut persistence, all_terms, path, &options)?;

    Ok(())
}

fn get_string_offsets(data: &Vec<&String>) -> Vec<u64> {
    let mut offsets = vec![];
    let mut offset = 0;
    for el in data {
        offsets.push(offset as u64);
        offset += el.len() + 1; // 1 for linevreak
    }
    offsets.push(offset as u64);
    offsets
}

#[derive(Debug, Clone)]
struct CharData {
    suffix: String,
    line_num: u64,
    byte_offset_start: u64,
}

impl PartialEq for CharData {
    fn eq(&self, other: &CharData) -> bool {
        self.suffix == other.suffix
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct TokenValueData {
    text: String,
    value: Option<u32>,
}

use search;
use search_field;
pub fn add_token_values_to_tokens(persistence: &mut Persistence, data_str: &str, config: &str) -> Result<(), search::SearchError> {
    let data: Vec<TokenValueData> = serde_json::from_str(data_str).unwrap();
    let config: TokenValuesConfig = serde_json::from_str(config).unwrap();

    let mut options: search::RequestSearchPart = search::RequestSearchPart {
        path: config.path.clone(),
        levenshtein_distance: Some(0),
        resolve_token_to_parent_hits: Some(false),

        ..Default::default()
    };

    let is_text_index = true;
    let path_name = util::get_file_path_name(&config.path, is_text_index);
    let mut tuples: Vec<ValIdToValue> = vec![];

    for el in data {
        if el.value.is_none() {
            continue;
        }
        options.terms = vec![el.text];
        options.terms = options.terms.iter().map(|el| util::normalize_text(el)).collect::<Vec<_>>();

        let hits = search_field::get_hits_in_field(persistence, &options, None)?;
        if hits.hits_vec.len() == 1 {
            tuples.push(ValIdToValue {
                valid: hits.hits_vec.iter().nth(0).unwrap().id,
                value: el.value.unwrap(),
            });
        }
    }
    persistence.write_boost_tuple_pair(&mut tuples, &concat(&path_name, ".tokenValues"))?;
    persistence.write_meta_data()?;
    Ok(())
}

pub fn create_indices_json(folder: &str, data: &Value, indices: &str) -> Result<(), CreateError> {
    info_time!(format!("total time create_indices for {:?}", folder));

    let indices_json: Vec<CreateIndex> = serde_json::from_str(indices).unwrap();
    let mut persistence = Persistence::create(folder.to_string())?;
    create_fulltext_index(data, &mut persistence, &indices_json)?;

    info_time!(format!("write json and metadata {:?}", folder));
    if let &Some(arr) = &data.as_array() {
        persistence.write_json_to_disk(arr, "data")?;
    } else {
        persistence.write_json_to_disk(&vec![data.clone()], "data")?;
    }

    persistence.write_meta_data()?;

    Ok(())
}

pub fn create_indices(folder: &str, data_str: &str, indices: &str) -> Result<(), CreateError> {
    let data: Value = serde_json::from_str(data_str).unwrap();
    create_indices_json(folder, &data, indices)
}

#[derive(Debug)]
pub enum CreateError {
    Io(io::Error),
    InvalidJson(serde_json::Error),
    Utf8Error(std::str::Utf8Error),
}

impl From<io::Error> for CreateError {
    fn from(err: io::Error) -> CreateError {
        CreateError::Io(err)
    }
}
impl From<serde_json::Error> for CreateError {
    fn from(err: serde_json::Error) -> CreateError {
        CreateError::InvalidJson(err)
    }
}
impl From<std::str::Utf8Error> for CreateError {
    fn from(err: std::str::Utf8Error) -> CreateError {
        CreateError::Utf8Error(err)
    }
}

// #[cfg(test)]
// mod test {
//     use create;
//     use serde_json;
//     use serde_json::Value;

//     #[test]
//     fn test_ewwwwwwwq() {

//         let opt: create::FulltextIndexOptions = serde_json::from_str(r#"{"tokenize":true, "stopwords": []}"#).unwrap();
//         // let opt = create::FulltextIndexOptions{
//         //     tokenize: true,
//         //     stopwords: vec![]
//         // };

//         let dat2 = r#" [{ "name": "John Doe", "age": 43 }, { "name": "Jaa", "age": 43 }] "#;
//         let data: Value = serde_json::from_str(dat2).unwrap();
//         let res = create::create_fulltext_index(&data, "name", opt);
//         let deserialized: create::BoostIndexOptions = serde_json::from_str(r#"{"boost_type":"int"}"#).unwrap();

//         assert_eq!("Hello", "Hello");

//         let service: create::CreateIndex = serde_json::from_str(r#"{"boost_type":"int"}"#).unwrap();

//     }
// }
