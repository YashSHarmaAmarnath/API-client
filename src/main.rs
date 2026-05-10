use reqwest::Client;
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
    println!("Hello, world!");

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
    let data: Value = get(&client, &endpoint, &baseurl).await?;

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
        body: "ahfhhhhfhdkd".into(),
        user_id: 1,
    };

    let response: Value = post(&client, &endpoint, &baseurl, &new_post).await?;

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
    )
    .await?;
    println!("patch: {:#?}", patched);

    // for DELETE
    delete(&client, "/delete", "http://127.0.0.1:5000/").await?;

    println!("Deleted");

    Ok(())
}

async fn get<T>(client: &Client, endpoint: &str, baseurl: &str) -> Result<T, reqwest::Error>
where
    T: DeserializeOwned,
{
    // let url = format!("{}{}", baseurl, endpoint);
    let url = url_builder(&baseurl,&endpoint,None);

    let data = client
        .get(url)
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
) -> Result<T, reqwest::Error>
where
    T: DeserializeOwned,
    B: Serialize,
{
    // let url = format!("{}{}", baseurl, endurl);
    let url = url_builder(&baseurl,&endpoint,None);
    client
        .post(url)
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
) -> Result<T, reqwest::Error>
where
    T: DeserializeOwned,
    B: Serialize,
{
    // let url = format!("{}{}", baseurl, endpoint);
    let url = url_builder(&baseurl,&endpoint,None);

    client
        .put(url)
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
) -> Result<T, reqwest::Error>
where
    T: DeserializeOwned,
    B: Serialize,
{
    // let url = format!("{}{}", baseurl, endpoint);
    let url = url_builder(&baseurl,&endpoint,None);

    client
        .patch(url)
        .json(body)
        .send()
        .await?
        .error_for_status()?
        .json::<T>()
        .await
}

async fn delete(client: &Client, endpoint: &str, baseurl: &str) -> Result<(), reqwest::Error> {
    // let url = format!("{}{}", baseurl, endpoint);
    let url = url_builder(&baseurl,&endpoint,None);
    client.delete(url).send().await?.error_for_status()?;
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

fn url_builder(baseurl:&str,endpoint: &str,query: Option<HashMap<String, String>>)->String{
    let mut url:String = format!("{}{}",baseurl,endpoint);
    if let Some(query_map)=query{
        let query_string = query_map.iter().map(|(k,v)|format!("{}={}",k,v)).collect::<Vec<_>>().join("&");
        url.push('?');
        url.push_str(&query_string);
    }
    return url;
}