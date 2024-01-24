use crate::{
    models::{
        ClientInfo, ConfigureIndexRequest, FetchRequest, FetchResponse, QueryRequest,
        QueryResponse, UpdateRequest,
    },
    rest::{try_pinecone_request_json, try_pinecone_request_text},
    Result,
};
use reqwest::{Method, StatusCode};
use serde_json::Value;

use super::{
    models::{IndexDescription, IndexStats, Metric, UpsertResponse, Vector, VectorRequest},
    Connection, Credentials,
};

impl From<Metric> for String {
    fn from(value: Metric) -> Self {
        value.to_string()
    }
}

/// Represents a connection to an Index. All Index specific operations are on this type.
pub struct Index {
    client: reqwest::Client,
    name: String,
    creds: Credentials,
    client_info: ClientInfo,
}

impl Index {
    pub(crate) fn new<C>(con: &C, name: impl Into<String>, client_info: &ClientInfo) -> Index
    where
        C: Connection,
    {
        Index {
            client: reqwest::Client::new(),
            name: name.into(),
            creds: con.credentials().clone(),
            client_info: client_info.clone(),
        }
    }

    /// Creates a brand new IndexDescription from pinecone.
    ///
    /// This method can also be used as a kind of Validation for you're credentials / Index. If it
    /// returns an Ok value the Index exists and if it returns an Error it likely does not.
    pub async fn describe(&self) -> Result<IndexDescription> {
        let name = self.name.clone();
        try_pinecone_request_json::<Index, String, IndexDescription>(
            self,
            Method::GET,
            StatusCode::OK,
            None::<String>,
            format!("/databases/{}", name),
            None,
        )
        .await
    }

    /// Returns the url for api requests if it's been cached, this is typically stored in
    /// [`IndexDescription`]
    pub fn url(&self) -> String {
        format!(
            "https://{}-{}.svc.{}.pinecone.io",
            self.name, self.client_info.project_name, self.creds.environment
        )
    }

    /// Grabs the latest [`IndexStats`] from pinecone.
    pub async fn describe_stats(&self) -> Result<IndexStats> {
        try_pinecone_request_json::<Index, String, IndexStats>(
            self,
            Method::GET,
            StatusCode::OK,
            Some(self.url()),
            "/describe_index_stats",
            None,
        )
        .await
    }

    /// Upsert takes in a [`Vec<Vector>`] and attempts to upsert / upload it to pinecone. It will
    /// return a [`UpsertResponse`] which is detailed in [Pinecone](https://docs.pinecone.io/reference/upsert)
    pub async fn upsert(&self, namespace: String, vectors: Vec<Vector>) -> Result<UpsertResponse> {
        let upsert = VectorRequest { namespace, vectors };
        try_pinecone_request_json::<Index, VectorRequest, UpsertResponse>(
            self,
            Method::POST,
            StatusCode::OK,
            Some(self.url()),
            "/vectors/upsert",
            Some(&upsert),
        )
        .await
    }

    /// Delete will attempt to delete the current Index and return the associated Message returned
    /// by Pinecone when successfull. This will error if the Index does not exist.
    pub async fn delete(self) -> Result<String> {
        try_pinecone_request_text::<Index, String>(
            &self,
            Method::DELETE,
            StatusCode::ACCEPTED,
            None::<String>,
            format!("/databases/{}", self.name),
            None,
        )
        .await
    }

    /// Configures the current index, specifically [`replicas`] and [`pod_type`] settings. More can
    /// be found at [Pinecone](https://docs.pinecone.io/reference/configure_index)
    pub async fn configure(&self, replicas: usize, pod_type: String) -> Result<String> {
        let p = ConfigureIndexRequest { replicas, pod_type };
        try_pinecone_request_text::<Index, ConfigureIndexRequest>(
            self,
            Method::PATCH,
            StatusCode::ACCEPTED,
            None::<String>,
            format!("/databases/{}", self.name),
            Some(&p),
        )
        .await
    }

    /// Updates a vector within the index. The return type of the Ok() value should be ignored as
    /// this method returns an empty json object.
    pub async fn update(&self, request: UpdateRequest) -> Result<Value> {
        try_pinecone_request_json::<Index, UpdateRequest, Value>(
            self,
            Method::POST,
            StatusCode::OK,
            Some(self.url()),
            "/vectors/update",
            Some(&request),
        )
        .await
    }

    /// Looksup and returns vectors, by ID, from a single namespace. The returned vectors
    /// include the vector data and/or metadata.
    pub async fn fetch(&self, request: FetchRequest) -> Result<FetchResponse> {
        let url = request.url(self.url());
        try_pinecone_request_json::<Index, String, FetchResponse>(
            self,
            Method::GET,
            StatusCode::OK,
            Some(url),
            "",
            None,
        )
        .await
    }

    /// Searches a namespace using a query vector. it retrieves the ids of the most similar items
    /// in a namespace, alogn with their similarity scores.
    pub async fn query(&self, request: QueryRequest) -> Result<QueryResponse> {
        try_pinecone_request_json::<Index, QueryRequest, QueryResponse>(
            self,
            Method::POST,
            StatusCode::OK,
            Some(self.url()),
            "/query",
            Some(&request),
        )
        .await
    }
}

impl Connection for Index {
    fn client(&self) -> &reqwest::Client {
        &self.client
    }
    fn credentials(&self) -> &Credentials {
        &self.creds
    }
}
#[cfg(test)]
mod index_tests {

    use super::*;
    use crate::{Client, Error};
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    async fn create_client() -> Client {
        Client::new(env!("PINECONE_API_KEY"), env!("PINECONE_ENV"))
            .await
            .unwrap()
    }

    async fn create_index(con: &Client) -> Index {
        Index::new(con, env!("PINECONE_INDEX_NAME"), con.info())
    }

    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    async fn test_upsert() {
        let client = create_client().await;
        let mut index = create_index(&client).await;
        let desc = match index.describe().await {
            Ok(desc) => desc,
            Err(err) => panic!("Unable to get dimension of index: {:?}", err),
        };
        let vec = Vector {
            id: "B".to_string(),
            values: vec![0.5; desc.database.dimension],
            sparse_values: None,
            metadata: None,
        };
        match index.upsert(String::from("halfbaked"), vec![vec]).await {
            Ok(_) => assert!(true),
            Err(err) => panic!("unable to upsert: {:?}", err),
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    async fn test_describe() {
        let client = create_client().await;
        let mut index = create_index(&client).await;
        match index.describe().await {
            Ok(_) => assert!(true),
            Err(err) => panic!("failed to get description: {:?}", err),
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    async fn test_describe_stats() {
        let client = create_client().await;
        let mut index = create_index(&client).await;
        match index.describe_stats().await {
            Ok(_) => assert!(true),
            Err(err) => panic!("failed to get index stats: {:?}", err),
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    async fn test_configure_index() {
        let client = create_client().await;
        let index = create_index(&client).await;
        match index.configure(1, "s1.x1".to_string()).await {
            Ok(_) => assert!(true),
            Err(error) => match error {
                Error::PineconeResponseError(code, typ, msg) => {
                    if code == StatusCode::BAD_REQUEST {
                        assert!(true);
                        return;
                    }
                    panic!(
                        "Unable to configure index: {:?}",
                        Error::PineconeResponseError(code, typ, msg)
                    )
                }
                _ => {
                    panic!("Unable to configure index: {:?}", error)
                }
            },
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    async fn test_update_index() {
        let client = create_client().await;
        let mut index = create_index(&client).await;
        let data = UpdateRequest {
            id: String::from("A"),
            ..Default::default()
        };
        match index.update(data).await {
            Ok(_) => assert!(true),
            Err(error) => match error {
                Error::PineconeResponseError(code, typ, msg) => {
                    if code == StatusCode::BAD_REQUEST {
                        assert!(true);
                        return;
                    }
                    panic!(
                        "Unable to configure index: {:?}",
                        Error::PineconeResponseError(code, typ, msg)
                    )
                }
                _ => {
                    panic!("Unable to configure index: {:?}", error)
                }
            },
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    async fn test_fetch_index() {
        let client = create_client().await;
        let mut index = create_index(&client).await;
        let data = FetchRequest {
            ids: vec!["A".to_string()],
            namespace: Some(String::from("halfbaked")),
        };
        match index.fetch(data).await {
            Ok(_) => assert!(true),
            Err(error) => panic!("Unable to fetch: {:?}", error),
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    async fn test_query_index() {
        let client = create_client().await;
        let mut index = create_index(&client).await;
        let data = QueryRequest {
            id: Some(String::from("A")),
            top_k: 1,
            ..Default::default()
        };
        match index.query(data).await {
            Ok(_) => assert!(true),
            Err(error) => panic!("Unable to fetch: {:?}", error),
        }
    }
}
