use reqwest::{Client};
use reqwest::header::{HeaderMap,HeaderValue};
// use serde::de::Error;
use std::error::Error;
use serde::{Serialize,};
use serde_json::Value;
use std::collections::HashMap;

mod utils;
use utils::get;
use utils::post;
use utils::put ;
use utils::patch ;
use utils::delete ;
// use utils::url_builder;
use utils::url_splitter;

#[derive(Debug, Serialize)]
struct NewPost {
    title: String,
    body: String,
    user_id: u32,
}

#[derive(Debug, Serialize)]
struct UpdatePost {
    title: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut query:HashMap<String, String> = HashMap::new();

    query.insert("userId".to_string(), "1".to_string());

    let mut headers = HeaderMap::new();

    headers.insert(
        "Authorization",
        HeaderValue::from_static("Bearer abc123"),
    );

    headers.insert(
        "Accept",
        HeaderValue::from_static("application/json"),
    );


    let client = Client::new();
    // for GET

    let url = "http://127.0.0.1:5000/get";

    let (baseurl, endpoint) = match url_splitter(url) {
        Some(parts) => parts,
        None => {
            println!("Invalid URL");
            return Ok(());
        }
    };
    let response= get::<Value>(&client, &endpoint, &baseurl,Some(&query),Some(headers),).await?;

    //for POST
    let url = "http://127.0.0.1:5000/post";
    let (baseurl, endpoint) = match url_splitter(url) {
        Some(parts) => parts,
        None => {
            println!("Invalid URL");
            return Ok(());
        }
    };
    let new_post = NewPost {
        title: "Hello".into(),
        body: "something".into(),
        user_id: 1,
        };
        
    let response = post::<Value,_>(&client, &endpoint, &baseurl, &new_post,Some(&query),None).await?;
        
    println!("{:#?}", response.body);
        
        // for PUT
    let url = "http://127.0.0.1:5000/put";
    let (baseurl, endpoint) = match url_splitter(url) {
        Some(parts) => parts,
        None => {println!("Invalid URL");
            return Ok(());
            }
    };
    let update = put::<Value,_>(&client,&endpoint,&baseurl,&UpdatePost {title: "Updated title".into(),},Some(&query),None).await?;
    println!("put: {:#?}", update.body);

    // for PATCH
    let url = "http://127.0.0.1:5000/patch";
    let (baseurl, endpoint) = match url_splitter(url) {
        Some(parts) => parts,
        None => {
            println!("Invalid URL");
            return Ok(());
        }
    };
    let patched = patch::<Value,_>(
        &client,
        &endpoint,
        &baseurl,
        &UpdatePost {
            title: "patched title".into(),
        },
        Some(&query),
        None
    )
    .await?;
    println!("patch: {:#?}", patched.body);


    // for DELETE
    let response = delete::<Value>(&client, "/delete", "http://127.0.0.1:5000/",Some(&query),None).await?;

    Ok(())
}