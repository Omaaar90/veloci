#![feature(plugin, custom_attribute)]
#![feature(underscore_lifetimes)]


#![cfg_attr(feature="flame_it", feature(plugin, custom_attribute))]
#![cfg_attr(feature="flame_it", plugin(flamer))]

#[cfg(feature="flame_it")]
extern crate flame;

extern crate bodyparser;
extern crate chashmap;
extern crate env_logger;
extern crate flexi_logger;
extern crate fnv;
extern crate hyper;
extern crate iron;
extern crate iron_cors;
extern crate multipart;
extern crate router;
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate snap;
extern crate time;
extern crate urlencoded;

extern crate iron_compress;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate measure_time;
extern crate search_lib;

#[allow(unused_imports)]
use iron_compress::GzipWriter;

use chashmap::CHashMap;

use search_lib::search;
use search_lib::search_field;
// use create;
// use doc_loader;
// use persistence;
use search_lib::persistence::Persistence;
use iron::prelude::*;
use iron::{typemap, AfterMiddleware, BeforeMiddleware, Chain, Iron, IronResult, Request, Response};
use iron_cors::CorsMiddleware;
use iron::{headers, status};
use iron::modifiers::Header;
use urlencoded::UrlEncodedQuery;

use multipart::server::{Entries, Multipart, SaveResult, SavedField};
use iron::mime::{SubLevel, TopLevel};

use time::precise_time_ns;
use router::Router;

use search_lib::persistence;

#[allow(unused_imports)]
use std::collections::HashMap;
#[allow(unused_imports)]
use fnv::FnvHashMap;
// use std::sync::RwLock;

use std::fs::File;
use std::io::prelude::*;

struct ResponseTime;

fn main() {
    // env_logger::init().unwrap();
    search_lib::trace::enable_log();
    // start_server("jmdict".to_string());
    start_server();
}

impl typemap::Key for ResponseTime {
    type Value = u64;
}

impl BeforeMiddleware for ResponseTime {
    fn before(&self, req: &mut Request) -> IronResult<()> {
        req.extensions.insert::<ResponseTime>(precise_time_ns());
        Ok(())
    }
}

impl AfterMiddleware for ResponseTime {
    fn after(&self, req: &mut Request, res: Response) -> IronResult<Response> {
        let delta = precise_time_ns() - *req.extensions.get::<ResponseTime>().unwrap();
        info!("Request took: {} ms", (delta as f64) / 1000000.0);
        Ok(res)
    }
}

use std::error::Error;
use std::fmt::{self, Debug};
#[derive(Debug)]
struct StringError(String);

impl fmt::Display for StringError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

impl Error for StringError {
    fn description(&self) -> &str {
        &*self.0
    }
}

// fn hello_world(_: &mut Request) -> IronResult<Response> {
//     Ok(Response::with((iron::status::Ok, "Hello World")))
// }
// const MAX_BODY_LENGTH: usize = 1024 * 1024 * 10;

lazy_static! {

    static ref PERSISTENCES: CHashMap<String, Persistence> = {
        CHashMap::default()
    };

}

fn ensure_database(database: &String) {
    if !PERSISTENCES.contains_key(database) {
        PERSISTENCES.insert(
            database.clone(),
            persistence::Persistence::load(database.clone()).expect("could not load persistence"),
        );
    }
}

fn extract_qp(req: &mut Request) -> Result<(HashMap<String, String>), IronError> {
    match req.get_ref::<UrlEncodedQuery>() {
        Ok(ref hashmap) => {
            info!("Parsed GET request query string:\n {:?}", hashmap);

            Ok(hashmap
                .iter()
                .map(|(key, ref val)| {
                    // TODO add error when size > 1
                    (key.clone(), val[0].clone())
                })
                .collect())

            // Ok(HashMap::default())
        }
        Err(ref e) => Err(IronError::new(
            StringError(e.to_string()),
            status::BadRequest,
        )),
    }
}

pub fn start_server() {
    // ensure_database(&database);
    // PERSISTENCES.write()

    for jeppo in std::env::args().skip(1) {
        ensure_database(&jeppo);
    }

    // &JMDICT_PERSISTENCE.print_heap_sizes();
    let mut router = Router::new(); // Alternative syntax:
    router.get("/", handler, "index"); // let router = router!(index: get "/" => handler,
    router.post("/:database/search", search_handler, "search");
    router.get("/:database/search", search_get_handler, "search_get");
    router.post("/:database/suggest", suggest_handler, "suggest");
    router.get("/:database/suggest", suggest_get_handler, "suggest");
    router.post("/:database/highlight", highlight_handler, "highlight");
    // let mut pers = Persistence::load("csv_test".to_string()).expect("Could not load persistence");

    router.post("/data/:database", handlero, "handlero");

    // Initialize middleware
    let cors_middleware = CorsMiddleware::with_allow_any();
    // Setup chain with middleware
    let mut chain = Chain::new(router);
    chain.link_around(cors_middleware);

    use std::env;

    let port = env::var("SERVER_PORT").unwrap_or("3000".to_string());
    let ip = env::var("SERVER_IP").unwrap_or("0.0.0.0".to_string());

    let combined = format!("{}:{}", ip, port);

    let yop = Iron::new(chain);
    // yop.threads = 1;
    println!("Start server {:?} Threads: {:?}", combined, yop.threads);
    yop.http(combined).unwrap();


    fn handler(_req: &mut Request) -> IronResult<Response> {
        let mut file = File::open("index.html").expect("Server: \"äh wo ist meine index.html\"");
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        Ok(Response::with((
            status::Ok,
            Header(headers::ContentType::html()),
            contents,
        )))
    }

    fn query_param_to_vec(name: &str, map: &HashMap<String, String>) -> Option<Vec<String>> {
        map.get(name)
            .clone()
            .map(|el| el.split(",").map(|f| f.to_string()).collect())
    }

    fn search_get_handler(req: &mut Request) -> IronResult<Response> {
        info_time!("search request total");
        let database = req.extensions
            .get::<Router>()
            .unwrap()
            .find("database")
            .expect("could not find collection name in url")
            .to_string();
        ensure_database(&database);

        // Extract the decoded data as hashmap, using the UrlEncodedQuery plugin.
        let map = extract_qp(req)?;
        // map.get("query").ok_or(IronError::new(
        //     StringError("query parameter not found".to_string()),
        //     status::BadRequest,
        // ))?;

        if map.get("query").is_none() {
            return Ok(Response::with((
                status::BadRequest,
                "query parameter not found".to_string(),
            )));
        }

        info!("get query {:?}", map.get("query").unwrap());

        // map.get("query").ok_or(IronError::new(
        //     StringError("query parameter not found".to_string()),
        //     status::BadRequest,
        // ))?;
        let persistence = PERSISTENCES.get(&database).unwrap();

        let facetlimit = map.get("facetlimit")
            .map(|el| el.parse::<usize>().unwrap())
            .clone();

        // map.get("facets").clone().split(",").map(|facet|{
        //     {"field":facet, top:facetlimit}
        // }).collect();
        let facets: Option<Vec<String>> = map.get("facets")
            .clone()
            .map(|el| el.split(",").map(|f| f.to_string()).collect());
        // "facets": [ {"field":"ISMLANGUAGES"}, {"field":"ISMARTIST"}, {"field":"GENRE"}, {"field":"VERLAG[]"}   ]
        let fields: Option<Vec<String>> = query_param_to_vec("fields", &map);
        let boost_fields: HashMap<String, f32> = query_param_to_vec("boost_fields", &map)
            .map(|mkay| {
                mkay.into_iter()
                    .map(|el| {
                        let field_n_boost = el.split("->").collect::<Vec<&str>>();
                        (
                            field_n_boost[0].to_string(),
                            field_n_boost[1].parse::<f32>().unwrap(),
                        )
                    })
                    .collect()
            })
            .unwrap_or(HashMap::default());

        let request = search::search_query(
            map.get("query").unwrap(),
            &persistence,
            map.get("top")
                .map(|el| el.parse::<usize>().unwrap())
                .clone(),
            map.get("skip")
                .map(|el| el.parse::<usize>().unwrap())
                .clone(),
            map.get("operator").map(|el| el.to_string()),
            map.get("levenshtein")
                .map(|el| el.parse::<usize>().unwrap())
                .clone(),
            facetlimit,
            facets,
            fields,
            boost_fields,
        );

        debug!("{}", serde_json::to_string(&request).unwrap());
        search_in_persistence(&persistence, request, enable_flame(req).unwrap_or(false))
    }

    fn suggest_get_handler(req: &mut Request) -> IronResult<Response> {
        info_time!("suggest request total");
        let database = req.extensions
            .get::<Router>()
            .unwrap()
            .find("database")
            .expect("could not find collection name in url")
            .to_string();
        ensure_database(&database);

        let map = extract_qp(req)?;

        if map.get("query").is_none() {
            return Ok(Response::with((
                status::BadRequest,
                "query parameter not found".to_string(),
            )));
        }

        info!("suggest query {:?}", map.get("query").unwrap());

        let query = map.get("query").ok_or(IronError::new(
            StringError("query parameter not found".to_string()),
            status::BadRequest,
        ))?;

        let persistence = PERSISTENCES.get(&database).unwrap();
        let fields: Option<Vec<String>> = query_param_to_vec("fields", &map);
        let request = search::suggest_query(
            query,
            &persistence,
            map.get("top")
                .map(|el| el.parse::<usize>().unwrap())
                .clone(),
            map.get("skip")
                .map(|el| el.parse::<usize>().unwrap())
                .clone(),
            map.get("levenshtein")
                .map(|el| el.parse::<usize>().unwrap())
                .clone(),
            fields,
        );

        let db = req.extensions
            .get::<Router>()
            .unwrap()
            .find("database")
            .expect("could not find collection name in url")
            .to_string();
        excute_suggest(request, db, enable_flame(req).unwrap_or(false))
    }

    fn get_body<T: 'static>(req: &mut Request) -> Result<T, IronError>
    where
        for<'a> T: serde::Deserialize<'a> + Clone + Debug,
    {
        let struct_body = req.get::<bodyparser::Struct<T>>();
        match struct_body {
            Ok(Some(struct_body)) => {
                // info!("Parsed body:\n{:?}", struct_body);
                // info_time!("search total");
                Ok(struct_body.clone())
            }
            Ok(None) => {
                info!("No body");
                Err(IronError::new(
                    StringError("No body".to_string()),
                    status::BadRequest,
                ))
            }
            Err(err) => {
                info!("Error: {:?}", err);
                Err(IronError::new(
                    StringError(err.to_string()),
                    status::BadRequest,
                ))
            }
        }
    }

    #[cfg_attr(feature="flame_it", flame)]
    fn search_in_persistence(persistence: &Persistence, request: search_lib::search::Request, enable_flame: bool) -> IronResult<Response> {
        // info!("Searching ... ");
        let hits = {
            info_time!("Searching ... ");
            let res = search::search(request, &persistence);
            if res.is_err() {
                return Ok(Response::with((
                    status::BadRequest,
                    format!("{:?}", res.unwrap_err()),
                )));

                // return Err(IronError::new(StringError(format!("ASDF ASDF {:?}", res.unwrap_err())), status::BadRequest));
            }
            res.unwrap()

            // search::search(request, &persistence).unwrap()
        };
        info!("Loading Documents... ");
        let doc = {
            info_time!("Loading Documents...  ");
            search::to_search_result(&persistence, hits)
        };

        debug!("Returning ... ");

        // Ok(Response::with((status::Ok, Header(headers::ContentType::json()), GzipWriter(serde_json::to_string(&doc).unwrap().as_bytes()) )))
        return_flame_or(enable_flame, GzipWriter(serde_json::to_string(&doc).unwrap().as_bytes()),)
    }

    fn search_handler(req: &mut Request) -> IronResult<Response> {
        let database = req.extensions
            .get::<Router>()
            .unwrap()
            .find("database")
            .expect("could not find collection name in url")
            .to_string();
        ensure_database(&database);
        // let ref query = req.extensions.get::<Router>().unwrap().find("query").unwrap_or("/");
        // Ok(Response::with(status::Ok))
        // Ok(Response::with((status::Ok, "*query")))
        info_time!("search total");
        let struct_body: search::Request = get_body(req)?;
        info!("Parsed body:\n{}", serde_json::to_string(&struct_body).unwrap());

        let persistence = PERSISTENCES.get(&database).unwrap();
        search_in_persistence(
            &persistence,
            struct_body,
            enable_flame(req).unwrap_or(false),
        )
    }

    fn suggest_handler(req: &mut Request) -> IronResult<Response> {
        let db = req.extensions
            .get::<Router>()
            .unwrap()
            .find("database")
            .expect("could not find collection name in url")
            .to_string();
        excute_suggest(get_body(req)?, db, enable_flame(req).unwrap_or(false))
    }
    fn excute_suggest(struct_body: search::Request, db: String, flame: bool) -> IronResult<Response> {
        ensure_database(&db);

        info_time!("search total");
        let persistence = PERSISTENCES.get(&db).unwrap();

        info!("Suggesting ... ");
        let hits = search_field::suggest_multi(&persistence, struct_body).unwrap();
        debug!("Returning ... ");
        // Ok(Response::with((status::Ok, Header(headers::ContentType::json()), serde_json::to_string(&hits).unwrap())))

        return_flame_or(flame, GzipWriter(serde_json::to_string(&hits).unwrap().as_bytes()),)
    }

    fn highlight_handler(req: &mut Request) -> IronResult<Response> {
        let database = req.extensions
            .get::<Router>()
            .unwrap()
            .find("database")
            .expect("could not find collection name in url")
            .to_string();
        ensure_database(&database);
        let mut struct_body: search::RequestSearchPart = get_body(req)?;
        info_time!("search total");
        let persistence = PERSISTENCES.get(&database).unwrap();

        info!("highlighting ... ");
        let hits = search_field::highlight(&persistence, &mut struct_body).unwrap();
        debug!("Returning ... ");
        // Ok(Response::with((status::Ok, Header(headers::ContentType::json()), serde_json::to_string(&hits).unwrap())))

        return_flame_or(
            enable_flame(req).unwrap_or(false),
            // serde_json::to_string(&hits).unwrap(),
            GzipWriter(serde_json::to_string(&hits).unwrap().as_bytes()),
        )
    }

    // create stuff
    fn handlero(req: &mut Request) -> IronResult<Response> {
        println!("getting 1 request");
        let header = req.headers
            .get::<headers::ContentType>()
            .expect("no content type set")
            .clone();

        println!("header: {:?}", *header);
        match *header {
            // iron::mime::Mime(TopLevel::Application, SubLevel::Json, _) => Ok(Response::with((status::BadRequest, "error"))),
            iron::mime::Mime(TopLevel::Application, iron::mime::SubLevel::WwwFormUrlEncoded, _) | iron::mime::Mime(TopLevel::Multipart, iron::mime::SubLevel::FormData, _) => {
                handle_multipart(req)
            }
            _ => {
                let error = format!(
                    "content type has to be {:?}/{:?} or {:?}/{:?} but got {:?}",
                    TopLevel::Application,
                    SubLevel::Json,
                    TopLevel::Multipart,
                    iron::mime::SubLevel::FormData,
                    *header
                );
                println!("Error: {:?}", error);
                Ok(Response::with((status::BadRequest, error)))
            }
        }
    }

    fn handle_multipart(req: &mut Request) -> IronResult<Response> {
        let database = req.extensions
            .get::<Router>()
            .unwrap()
            .find("database")
            .expect("could not find collection name in url")
            .to_string();
        let enable_flame = enable_flame(req).unwrap_or(false);
        println!("handle_multipart: {:?}", &database);
        match Multipart::from_request(req) {
            Ok(mut multipart) => {
                // Fetching all data and processing it.
                // save().temp() reads the request fully, parsing all fields and saving all files
                // in a new temporary directory under the OS temporary directory.
                match multipart.save().temp() {
                    SaveResult::Full(entries) => handle_db_insert(enable_flame, entries, database),
                    SaveResult::Partial(_, reason) => Ok(Response::with((
                        status::BadRequest,
                        format!("error reading request: {}", reason.unwrap_err()),
                    ))),
                    SaveResult::Error(error) => Ok(Response::with((
                        status::BadRequest,
                        format!("error reading request: {}", error),
                    ))),
                }
            }
            Err(err) => {
                println!("Error: {:?}", err);
                Ok(Response::with((
                    status::BadRequest,
                    "The request is not multipart?",
                )))
            }
        }
    }

    fn enable_flame(req: &mut Request) -> Result<bool, IronError> {
        let map = extract_qp(req)?;
        Ok(map.get("flame").is_some())
    }

    #[cfg_attr(feature="flame_it", flame)]
    fn handle_db_insert(enable_flame: bool, entries: Entries, database: String) -> IronResult<Response> {
        if entries.fields.len() != 1 {
            return Ok(Response::with((
                status::BadRequest,
                format!(
                    "only single file uploads supported, but got {} entries",
                    entries.fields.len()
                ),
            )));
        }

        let entry = entries.fields.iter().last().unwrap();
        println!("Field {:?} has {} fields:", entry.0, entry.1.len());
        if entry.1.len() != 1 {
            return Ok(Response::with((
                status::BadRequest,
                "only single file uploads supported",
            )));
        }
        let contents = get_multipart_file_contents(&entry.1.iter().last().unwrap())?;

        let indices = r#"[] "#;

        println!(
            "{:?}",
            search_lib::create::create_indices(&database, &contents, indices)
        );

        // {
        //     let mut pers = persistence::Persistence::load(database.to_string()).expect("Could not load persistence");
        //     // let mut pers = persistence::Persistence::load(database.to_string()).expect("Could not load persistence");
        //     let config = json!({
        //         "path": "meanings.ger[]"
        //     });
        //     create::add_token_values_to_tokens(&mut pers, TOKEN_VALUE, &config.to_string()).expect("Could not add token values");
        // }
        PERSISTENCES.insert(
            database.clone(),
            persistence::Persistence::load(database.to_string()).expect("could not load persistence"),
        );

        return_flame_or(
            enable_flame,
            GzipWriter(serde_json::to_string(&json!({"result": "database created"})).unwrap().as_bytes()),
            // serde_json::to_string(&json!({"result": "database created"})).unwrap(),
        )
    }

    #[cfg(flame_it)]
    fn return_flame_or(enable_flame: bool, content: GzipWriter) -> IronResult<Response> {
        if enable_flame{
            let mut flame = vec![];
            flame::dump_html(&mut flame).unwrap();
            Ok(Response::with((
                status::Ok,
                Header(headers::ContentType::html()),
                String::from_utf8(flame).unwrap(),
            )))
        } else {
            Ok(Response::with((
                status::Ok,
                Header(headers::ContentType::json()),
                content,
            )))
        }
    }

    #[cfg(not(flame_it))]
    fn return_flame_or(_enable_flame: bool, content: GzipWriter) -> IronResult<Response> {
        Ok(Response::with((
            status::Ok,
            Header(headers::ContentType::json()),
            content,
        )))
    }

    fn get_multipart_file_contents(saved_file: &SavedField) -> IronResult<(String)> {
        let mut contents = String::new();
        // saved_file.data.readable();

        if let Err(error) = saved_file.data.readable().unwrap().read_to_string(&mut contents) {
            return Err(IronError::new(
                error,
                (status::BadRequest, "The file was not a text"),
            ));
        }
        println!(
            "File {:?} ({:?}):",
            saved_file.headers.filename, saved_file.headers.content_type
        );
        Ok(contents)


        // let mut file = match File::open(&saved_file.path) {
        //     Ok(file) => file,
        //     Err(error) => {
        //         return Err(IronError::new(
        //             error,
        //             (
        //                 status::InternalServerError,
        //                 "Server couldn't open saved file",
        //             ),
        //         ))
        //     }
        // };

        // let mut contents = String::new();
        // if let Err(error) = file.read_to_string(&mut contents) {
        //     return Err(IronError::new(
        //         error,
        //         (status::BadRequest, "The file was not a text"),
        //     ));
        // }
        // println!(
        //     "File {:?} ({:?}):",
        //     saved_file.filename, saved_file.content_type
        // );
        // Ok(contents)
    }
}
