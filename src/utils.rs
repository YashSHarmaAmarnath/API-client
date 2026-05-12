// use reqwest::header;
// use reqwest::{Client};
use reqwest::{Client,Response, StatusCode, Version, header::HeaderMap};
// use reqwest::header::{HeaderMap};
use serde::{Serialize, de::DeserializeOwned};
// use serde_json::value::Index;
// use tokio::io::DuplexStream;
use std::collections::HashMap;
use std::time::Instant;
use std::{error::Error,time::Duration};

#[derive(Debug)]
pub struct ApiResponse<T>{
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: T,
    pub raw_text: String,
    pub final_url: String,
    pub content_length: Option<u64>,
    pub http_version: Version,
    pub response_time: Duration,
}

async fn handle_response<T>(
    response:Response,
    response_time: Duration
) -> Result<ApiResponse<T>,Box<dyn Error>>
where 
    T: DeserializeOwned,
{
    let status = response.status();
    let headers = response.headers().clone();
    let final_url = response.url().to_string();
    let content_length = response.content_length();
    let http_version = response.version();
    
    let raw_text = response.text().await?;

    let body: T = serde_json::from_str(&raw_text)?;

    Ok(ApiResponse { status, headers, body, raw_text, final_url, content_length, http_version, response_time })
}

pub async fn get<T>(
    client: &Client,
    endpoint: &str,
    baseurl: &str,
    query: Option<&HashMap<String,String>>,
    header:Option<HeaderMap> 
) -> Result<ApiResponse<T>, Box<dyn Error>>
where
    T: DeserializeOwned,
{
    let url = url_builder(baseurl,endpoint,query);

    let mut request = client.get(url);

    if let Some(h) = header{
        request = request.headers(h);
    }

    let start = Instant::now();
    let response   = request.send().await?;

    handle_response(response, start.elapsed()).await
}

pub async fn post<T, B>(
    client: &Client,
    endpoint: &str,
    baseurl: &str,
    body: &B,
    query: Option<&HashMap<String, String>>
    ,header:Option<HeaderMap> 
) -> Result<ApiResponse<T>, Box<dyn Error>>
where
    T: DeserializeOwned,
    B: Serialize,
{
    let url = url_builder(baseurl,endpoint,query);
    let mut request = client.post(url).json(body);

    if let Some(h) = header{
        request = request.headers(h);
    }
    let start = Instant::now();
    let response = request.send().await?;
    handle_response(response, start.elapsed()).await
}

pub async fn put<T, B>(
    client: &Client,
    endpoint: &str,
    baseurl: &str,
    body: &B,
    query: Option<&HashMap<String, String>>
    ,header:Option<HeaderMap> 
) -> Result<ApiResponse<T>, Box<dyn Error>>
where
    T: DeserializeOwned,
    B: Serialize,
{
    let url = url_builder(baseurl,endpoint,query);
    let mut request = client.put(url).json(body);

    if let Some(h) = header{
        request = request.headers(h);
    }
    let start = Instant::now();
    let response = request.send().await?;

    handle_response(response, start.elapsed()).await
}

pub async fn patch<T, B>(
    client: &Client,
    endpoint: &str,
    baseurl: &str,
    body: &B,
    query: Option<&HashMap<String, String>>
    ,header:Option<HeaderMap> 
) -> Result<ApiResponse<T>, Box<dyn Error>>
where
    T: DeserializeOwned,
    B: Serialize,
{
    let url = url_builder(&baseurl,&endpoint,query);
    let mut request = client.patch(url).json(body);

    if let Some(h) = header{
        request = request.headers(h);
    }
    let start = Instant::now();
    let response = request.send().await?;
    
    handle_response(response, start.elapsed()).await
}

pub async fn delete<T>(
    client: &Client, 
    endpoint: &str, 
    baseurl: &str,
    query: Option<&HashMap<String, String>>,
    header:Option<HeaderMap>
) -> Result<ApiResponse<T>, Box<dyn Error>>
where
    T: DeserializeOwned,
{
    let url = url_builder(baseurl,endpoint,query);
    let mut request = client.delete(url);

    if let Some(h) = header{
        request = request.headers(h);
    }
    let start = Instant::now();
    let response = request.send().await?.error_for_status()?;

    handle_response(response, start.elapsed()).await
}

pub fn url_splitter(url: &str) -> Option<(String, String)> {
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

pub fn url_builder(baseurl:&str,endpoint: &str,query: Option<&HashMap<String, String>>)->String{
    let mut url:String = format!("{}{}",baseurl,endpoint);
    if let Some(query_map)=query{
        let query_string = query_map.iter().map(|(k,v)|format!("{}={}",k,v)).collect::<Vec<_>>().join("&");
        url.push('?');
        url.push_str(&query_string);
    }
    url
}

