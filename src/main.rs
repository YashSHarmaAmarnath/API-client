use reqwest::Client;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize)]
struct NewPost {
    title: String,
    body: String,
    user_id: u32,
}

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    println!("Hello, world!");
    
    let client = Client::new();
    // for GET
    
    // let url = "https://jsonplaceholder.typicode.com/posts/1";
    let url = "http://127.0.0.1:5000/get";

    let (baseurl,endpoint)  = match url_parser(url){
        Some(parts)=>parts,
        None => {
            println!("Invalid URL");
            return Ok(());
        }
    };
    let data: Value = get(&client,&endpoint,&baseurl).await?;
    
    println!("{:#}",data);

    //for POST
    let url = "http://127.0.0.1:5000/post";
    let (baseurl,endpoint)  = match url_parser(url){
        Some(parts)=>parts,
        None => {
            println!("Invalid URL");
            return Ok(());
        }
    };
    let new_post = NewPost{
        title:"Hello".into(),
        body : "ahfhhhhfhdkd".into(),
        user_id: 1,
    };

    let response :Value = post(&client, &endpoint, &baseurl, &new_post).await?;

    println!("{:#?}",response);

    Ok(())
}

async fn get<T>(client: &Client, endpoint: &str,baseurl: &str)->Result<T, reqwest::Error>
where
    T:DeserializeOwned,
{
        let url = format!("{}{}",baseurl,endpoint);

        let data = client.get(url).send().await?.error_for_status()?.json::<T>().await?;

        Ok(data)
}

async fn post<T,B>(client: &Client,endurl: &str,baseurl:&str,body:&B)->Result<T, reqwest::Error>
where 
    T: DeserializeOwned,
    B: Serialize,
{
    let url = format!("{}{}",baseurl,endurl);

    client.post(url).json(body).send().await?.error_for_status()?.json::<T>().await
}

fn url_parser(url:&str)->Option<(String,String)>{
    let parts: Vec<&str> = url.splitn(4,'/').collect();
    
    if parts.len()<3{
        return None;
    }

    let baseurl = format!("{}//{}",parts[0],parts[2]);
    let endpoint = if parts.len()>3{
        format!("/{}",parts[3])
    } else {
        "/".to_string()
    };
    Some((baseurl,endpoint))    
}