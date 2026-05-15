use rusqlite::{Connection,Result, params};
use serde::{Deserialize,Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct Collection {
    pub collection_id: i64,
    pub collection_name: String,
    pub description: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiRequest {
    request_id: i64,
    collection_id: i64,
    request_name: String,
    method: String,
    url: String,

    headers: Option<HashMap<String, String>>,

    body: Option<serde_json::Value>,
}

pub fn check_database(conn:&Connection)->Result<()>{
    
    conn.execute("PRAGMA foreign_keys = ON", [])?;

    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS collections (
            collection_id INTEGER PRIMARY KEY AUTOINCREMENT,
            collection_name TEXT NOT NULL,
            description TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS api_requests (
            request_id INTEGER PRIMARY KEY AUTOINCREMENT,
            collection_id INTEGER NOT NULL,
            request_name TEXT NOT NULL,
            method TEXT NOT NULL,
            url TEXT NOT NULL,
            headers TEXT,
            body TEXT,

            FOREIGN KEY (collection_id)
                REFERENCES collections(collection_id)
                ON DELETE CASCADE
        );
        "
    )?;

    Ok(())
}

pub fn db_create_collection(conn:&Connection,coll_name:&str,desc:&str)->Result<()>{
    conn.execute("
    INSERT INTO collections (
        collection_name,
        description
    )
    VALUES (?1,?2)
    ", (coll_name,desc),
    )?;
    Ok(())
}

pub fn db_insert_api_request(
    conn: &Connection,
    collection_id: i64,
    request_name: &str,
    method: &str,
    url: &str,
    headers: Option<&HashMap<String, String>>,
    body: Option<&serde_json::Value>,
) -> Result<()> {

    let headers_json = headers.map(|h|{
        serde_json::to_string(h).unwrap()
    });

    let body_json = body.map(|b|{
        serde_json::to_string(b).unwrap()
    });

    conn.execute("
    INSERT INTO api_requests (
        collection_id,
        request_name,
        method,
        url,
        headers,
        body
    )
    VALUES (?1,?2,?3,?4,?5,?6)
    ", params![
            collection_id,
            request_name,
            method,
            url,
            headers_json,
            body_json])?;

    Ok(())
}

pub fn get_all_collections(
    conn: &Connection,
) -> Result<Vec<Collection>> {

    let mut stmt = conn.prepare(
        "
        SELECT
            collection_id,
            collection_name,
            description,
            created_at
        FROM collections
        "
    )?;

    let collections_iter = stmt.query_map([], |row| {
        Ok(Collection {
            collection_id: row.get(0)?,
            collection_name: row.get(1)?,
            description: row.get(2)?,
            created_at: row.get(3)?,
        })
    })?;

    let collections: Result<Vec<_>> =
        collections_iter.collect();

    collections
}


pub fn get_requests_by_collection_id(
    conn: &Connection,
    collection_id: i64,
) -> Result<Vec<ApiRequest>> {

    let mut stmt = conn.prepare(
        "
        SELECT
            request_id,
            collection_id,
            request_name,
            method,
            url,
            headers,
            body
        FROM api_requests
        WHERE collection_id = ?1
        "
    )?;

    let requests_iter = stmt.query_map(
        [collection_id],
        |row| {

            let headers_json: Option<String> =
                row.get(5)?;

            let body_json: Option<String> =
                row.get(6)?;

            let headers = headers_json.map(|h| {
                serde_json::from_str(&h).unwrap()
            });

            let body = body_json.map(|b| {
                serde_json::from_str(&b).unwrap()
            });

            Ok(ApiRequest {
                request_id: row.get(0)?,
                collection_id: row.get(1)?,
                request_name: row.get(2)?,
                method: row.get(3)?,
                url: row.get(4)?,
                headers,
                body,
            })
        },
    )?;

    let requests: Result<Vec<_>> =
        requests_iter.collect();

    requests
}

pub fn db_delete_collection(
    conn:&Connection,
    collection_id: i64
)->Result<()>
{    
    conn.execute("
        DELETE FROM collections
        WHERE collection_id = ?1
    ", [collection_id])?;

    Ok(())
}

pub fn db_delete_request(
    conn:&Connection,
    request_id:i64
)->Result<()>
{   
    conn.execute("
        delete from api_requests
        where request_id = ?1
    ", [request_id])?;
    
    Ok(())
}