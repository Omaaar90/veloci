#![feature(plugin)]
#![cfg_attr(test, plugin(stainless))]
#![recursion_limit = "128"]

extern crate env_logger;
#[macro_use]
extern crate lazy_static;
extern crate search_lib;
#[macro_use]
extern crate serde_json;

use search_lib::create;
use search_lib::persistence;
use search_lib::search;
use search_lib::trace;
use serde_json::Value;
use search_lib::query_generator;

pub fn get_test_data() -> Value {
    json!([
        {
            "title": "die erbin"
        },
        {
            "title": "erbin",
            "tags": ["die", "erbin"]
        },
        {
            "tags": ["greg tagebuch 05"]
        },
        {
            "tags": ["greg tagebuch", "05"]
        }
    ])
}

static TEST_FOLDER: &str = "mochaTest_phrase";

lazy_static! {
    static ref TEST_PERSISTENCE:persistence::Persistence = {
        trace::enable_log();
        let indices = r#"[{ "fulltext":"title", "options":{"tokenize":true} } ] "#;
        let mut persistence = persistence::Persistence::create(TEST_FOLDER.to_string()).unwrap();

        let data = get_test_data();
        if let Some(arr) = data.as_array() {
            let docs_line_separated = arr.iter().fold(String::with_capacity(100), |acc, el| acc + &el.to_string()+"\n");
            println!("{:?}", create::create_indices_from_str(&mut persistence, &docs_line_separated, indices, None, false));
        }

        let pers = persistence::Persistence::load(TEST_FOLDER.to_string()).expect("Could not load persistence");
        pers
    };
}

fn search_testo_to_doco_qp(qp: query_generator::SearchQueryGeneratorParameters) -> search::SearchResultWithDoc {
    let pers = &TEST_PERSISTENCE;
    let requesto = query_generator::search_query(&pers, qp);
    search::to_search_result(&pers, search_testo_to_hitso(requesto.clone()).expect("search error"), &requesto.select)
}
fn search_testo_to_doc(req: Value) -> search::SearchResultWithDoc {
    let pers = &TEST_PERSISTENCE;
    let requesto: search::Request = serde_json::from_str(&req.to_string()).expect("Can't parse json");
    search::to_search_result(&pers, search_testo_to_hitso(requesto).expect("search error"), &None)
}

fn search_testo_to_hitso(requesto: search::Request) -> Result<search::SearchResult, search::SearchError> {
    let pers = &TEST_PERSISTENCE;
    let hits = search::search(requesto, &pers)?;
    Ok(hits)
}

describe! search_test {

    it "should boost phrase"{
        let req = json!({
            "search": {"terms":["erbin"], "path": "title"},
            "phrase_boosts": [{
                "path":"title",
                "search1":{"terms":["die"], "path": "title"},
                "search2":{"terms":["erbin"], "path": "title"}
            }]
        });

        let hits = search_testo_to_doc(req).data;
        assert_eq!(hits[0].doc["title"], "die erbin");
    }

    it "should boost phrase search multifield"{
        let req = json!({
            "or":[
                {"search": {"terms":["die"], "path": "title" }},
                {"search": {"terms":["erbin"], "path": "title" }},
                {"search": {"terms":["die"], "path": "tags[]" }},
                {"search": {"terms":["erbin"], "path": "tags[]" }}
            ],
            "phrase_boosts": [{
                "path":"title",
                "search1":{"terms":["die"], "path": "title" },
                "search2":{"terms":["erbin"], "path": "title" }
            },{
                "path":"tags[]",
                "search1":{"terms":["die"], "path": "tags[]" },
                "search2":{"terms":["erbin"], "path": "tags[]" }
            }]
        });

        let hits = search_testo_to_doc(req).data;
        assert_eq!(hits[0].doc["title"], "die erbin");
    }

    it "should and boost phrase search"{
        let req = json!({
            "and":[
                {"search": {"terms":["die"], "path": "title" }},
                {"search": {"terms":["erbin"], "path": "title" }}
            ],
            "phrase_boosts": [{
                "path":"title",
                "search1":{"terms":["die"], "path": "title" },
                "search2":{"terms":["erbin"], "path": "title" }
            }]
        });

        let hits = search_testo_to_doc(req).data;
        assert_eq!(hits[0].doc["title"], "die erbin");
    }

    it "should and boost phrase AND query generator"{
        let mut params = query_generator::SearchQueryGeneratorParameters::default();
        params.search_term="die AND erbin".to_string();
        params.phrase_pairs = Some(true);
        let hits = search_testo_to_doco_qp(params).data;
        assert_eq!(hits[0].doc["title"], "die erbin");
    }

    it "should and boost phrase OR query generator"{
        let mut params = query_generator::SearchQueryGeneratorParameters::default();
        params.search_term="die erbin".to_string();
        params.phrase_pairs = Some(true);
        let hits = search_testo_to_doco_qp(params).data;
        assert_eq!(hits[0].doc["title"], "die erbin");
    }

    it "should double boost from multiphrases"{
        let req_with_single_phrase = json!({
            "or":[
                {"search": {"terms":["greg"], "path": "tags[]" }},
                {"search": {"terms":["tagebuch"], "path": "tags[]" }},
                {"search": {"terms":["05"], "path": "tags[]" }}
            ],
            "phrase_boosts": [{
                "path":"tags[]",
                "search1":{"terms":["greg"], "path": "tags[]" },
                "search2":{"terms":["tagebuch"], "path": "tags[]" }
            }]
        });

        let hits = search_testo_to_doc(req_with_single_phrase).data;
        assert_eq!(hits[0].doc["tags"][0], "greg tagebuch");

        let req_with_multi_phrase = json!({
            "or":[
                {"search": {"terms":["greg"], "path": "tags[]" }},
                {"search": {"terms":["tagebuch"], "path": "tags[]" }},
                {"search": {"terms":["05"], "path": "tags[]" }}
            ],
            "phrase_boosts": [{
                "path":"tags[]",
                "search1":{"terms":["greg"], "path": "tags[]" },
                "search2":{"terms":["tagebuch"], "path": "tags[]" }
            },{
                "path":"tags[]",
                "search1":{"terms":["tagebuch"], "path": "tags[]" },
                "search2":{"terms":["05"], "path": "tags[]" }
            }]
        });

        let hits = search_testo_to_doc(req_with_multi_phrase).data;
        assert_eq!(hits[0].doc["tags"][0], "greg tagebuch 05");

    }
    it "should double boost from multiphrases AND searchterms"{
        let req_with_single_phrase = json!({
            "and":[
                {"search": {"terms":["greg"], "path": "tags[]" }},
                {"search": {"terms":["tagebuch"], "path": "tags[]" }},
                {"search": {"terms":["05"], "path": "tags[]" }}
            ],
            "phrase_boosts": [{
                "path":"tags[]",
                "search1":{"terms":["greg"], "path": "tags[]" },
                "search2":{"terms":["tagebuch"], "path": "tags[]" }
            }]
        });

        let hits = search_testo_to_doc(req_with_single_phrase).data;
        assert_eq!(hits[0].doc["tags"][0], "greg tagebuch");

        let req_with_multi_phrase = json!({
            "and":[
                {"search": {"terms":["greg"], "path": "tags[]" }},
                {"search": {"terms":["tagebuch"], "path": "tags[]" }},
                {"search": {"terms":["05"], "path": "tags[]" }}
            ],
            "phrase_boosts": [{
                "path":"tags[]",
                "search1":{"terms":["greg"], "path": "tags[]" },
                "search2":{"terms":["tagebuch"], "path": "tags[]" }
            },{
                "path":"tags[]",
                "search1":{"terms":["tagebuch"], "path": "tags[]" },
                "search2":{"terms":["05"], "path": "tags[]" }
            }]
        });

        let hits = search_testo_to_doc(req_with_multi_phrase).data;
        assert_eq!(hits[0].doc["tags"][0], "greg tagebuch 05");

    }

}