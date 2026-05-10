use reqwest::{Client};
use reqwest::header::{HeaderMap,HeaderValue};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::HashMap;

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
async fn main() -> Result<(), reqwest::Error> {
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

    // let url = "https://jsonplaceholder.typicode.com/posts/1";
    let url = "http://127.0.0.1:5000/get";

    let (baseurl, endpoint) = match url_splitter(url) {
        Some(parts) => parts,
        None => {
            println!("Invalid URL");
            return Ok(());
        }
    };
    let data: Value = get(&client, &endpoint, &baseurl,Some(&query),Some(headers),).await?;

    println!("{:#}", data);

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

    let response: Value = post(&client, &endpoint, &baseurl, &new_post,Some(&query),None).await?;

    println!("{:#?}", response);

    // for PUT
    let url = "http://127.0.0.1:5000/put";
    let (baseurl, endpoint) = match url_splitter(url) {
        Some(parts) => parts,
        None => {
            println!("Invalid URL");
            return Ok(());
        }
    };
    let update: Value = put(
        &client,
        &endpoint,
        &baseurl,
        &UpdatePost {
            title: "Updated title".into(),
        },
        Some(&query),
        None
    )
    .await?;
    println!("put: {:#?}", update);

    // for PATCH
    let url = "http://127.0.0.1:5000/patch";
    let (baseurl, endpoint) = match url_splitter(url) {
        Some(parts) => parts,
        None => {
            println!("Invalid URL");
            return Ok(());
        }
    };
    let patched: Value = patch(
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
    println!("patch: {:#?}", patched);

    // for DELETE
    delete(&client, "/delete", "http://127.0.0.1:5000/",Some(&query),None).await?;

    println!("Deleted");

    Ok(())
}

async fn get<T>(client: &Client, endpoint: &str, baseurl: &str,query: Option<&HashMap<String, String>>,header:Option<HeaderMap> ) -> Result<T, reqwest::Error>
where
    T: DeserializeOwned,
{
    // let url = format!("{}{}", baseurl, endpoint);
    let url = url_builder(baseurl,endpoint,query);

    let mut request = client.get(url);

    if let Some(h) = header{
        request = request.headers(h);
    }

    let data = request
        .send()
        .await?
        .error_for_status()?
        .json::<T>()
        .await?;

    Ok(data)
}

async fn post<T, B>(
    client: &Client,
    endpoint: &str,
    baseurl: &str,
    body: &B,
    query: Option<&HashMap<String, String>>
    ,header:Option<HeaderMap> 
) -> Result<T, reqwest::Error>
where
    T: DeserializeOwned,
    B: Serialize,
{
    // let url = format!("{}{}", baseurl, endpoint);
    let url = url_builder(baseurl,endpoint,query);
    let mut request = client.post(url);

    if let Some(h) = header{
        request = request.headers(h);
    }
    request
        .json(body)
        .send()
        .await?
        .error_for_status()?
        .json::<T>()
        .await
}

async fn put<T, B>(
    client: &Client,
    endpoint: &str,
    baseurl: &str,
    body: &B,
    query: Option<&HashMap<String, String>>
    ,header:Option<HeaderMap> 
) -> Result<T, reqwest::Error>
where
    T: DeserializeOwned,
    B: Serialize,
{
    // let url = format!("{}{}", baseurl, endpoint);
    let url = url_builder(baseurl,endpoint,query);
    let mut request = client.put(url);

    if let Some(h) = header{
        request = request.headers(h);
    }
    request
        .json(body)
        .send()
        .await?
        .error_for_status()?
        .json::<T>()
        .await
}
async fn patch<T, B>(
    client: &Client,
    endpoint: &str,
    baseurl: &str,
    body: &B,
    query: Option<&HashMap<String, String>>
    ,header:Option<HeaderMap> 
) -> Result<T, reqwest::Error>
where
    T: DeserializeOwned,
    B: Serialize,
{
    // let url = format!("{}{}", baseurl, endpoint);
    let url = url_builder(&baseurl,&endpoint,query);
    let mut request = client.patch(url);

    if let Some(h) = header{
        request = request.headers(h);
    }
    request
        .json(body)
        .send()
        .await?
        .error_for_status()?
        .json::<T>()
        .await
}

async fn delete(client: &Client, endpoint: &str, baseurl: &str,query: Option<&HashMap<String, String>>,header:Option<HeaderMap>  ) -> Result<(), reqwest::Error> {
    // let url = format!("{}{}", baseurl, endpoint);
    let url = url_builder(baseurl,endpoint,query);
    let mut request = client.delete(url);

    if let Some(h) = header{
        request = request.headers(h);
    }
    request.send().await?.error_for_status()?;
    Ok(())
}

fn url_splitter(url: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = url.splitn(4, '/').collect();

    if parts.len() < 3 {
        return None;
    }

    let baseurl = format!("{}//{}", parts[0], parts[2]);
    let endpoint = if parts.len() > 3 {
        format!("/{}", parts[3])
    } else {
        "/".to_string()
    };
    Some((baseurl, endpoint))
}

fn url_builder(baseurl:&str,endpoint: &str,query: Option<&HashMap<String, String>>)->String{
    let mut url:String = format!("{}{}",baseurl,endpoint);
    if let Some(query_map)=query{
        let query_string = query_map.iter().map(|(k,v)|format!("{}={}",k,v)).collect::<Vec<_>>().join("&");
        url.push('?');
        url.push_str(&query_string);
    }
    url
}