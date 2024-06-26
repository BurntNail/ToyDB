//! `async_client` provides an asynchronous client for use with a `sourisd` client.
//!
//! When you create a new client using [`AsyncClient::new`], it polls the database's healthcheck endpoint to confirm that the database is running.
//!
//! ```rust
//! use sourisdb::client::{AsyncClient, ClientError};
//!
//! async fn get_all_database_names_from_localhost () -> Result<Vec<String>, ClientError> {
//!     let client = AsyncClient::new("localhost", 2256).await?;
//!     client.get_all_dbs()
//! }
//! ```

use core::fmt::Display;

use http::StatusCode;
use reqwest::{Client, Response};

use crate::{client::ClientError, store::Store, values::Value};

///A client for interacting with `sourisd` asynchronously.
#[derive(Debug, Clone)]
pub struct AsyncClient {
    path: String,
    port: u32,
    client: Client,
}

impl AsyncClient {
    ///Create a new asynchronous client using the provided path and port.
    ///
    /// ## Errors
    /// Can fail with:
    /// - [`ClientError::Reqwest`] if there
    pub async fn new(path: impl Display, port: u32) -> Result<Self, ClientError> {
        let path = path.to_string();
        let client = Client::new();

        match client
            .get(&format!("http://{path}:{port}/healthcheck"))
            .send()
            .await
        {
            Ok(rsp) => {
                if rsp.status() != StatusCode::OK {
                    return Err(ClientError::ServerNotHealthy(rsp.status()));
                }
            }
            Err(e) => {
                if let Some(status) = e.status() {
                    if status != StatusCode::OK {
                        return Err(ClientError::ServerNotHealthy(status));
                    }
                } else {
                    return Err(ClientError::Reqwest(e));
                }
            }
        };

        Ok(Self { path, port, client })
    }

    pub async fn get_all_dbs(&self) -> Result<Vec<String>, ClientError> {
        let rsp = self
            .client
            .get(&format!(
                "http://{}:{}/v1/get_all_db_names",
                self.path, self.port
            ))
            .send()
            .await?;
        rsp.error_for_status_to_client_error()?;
        let body = rsp.bytes().await?;
        Ok(serde_json::from_slice(body.as_ref())?)
    }

    pub async fn create_new_db(
        &self,
        overwrite_existing: bool,
        name: &str,
    ) -> Result<bool, ClientError> {
        let rsp = self
            .client
            .post(&format!("http://{}:{}/v1/add_db", self.path, self.port))
            .query(&[
                (
                    "overwrite_existing",
                    if overwrite_existing { "true" } else { "false" },
                ),
                ("db_name", name),
            ])
            .send()
            .await?;
        Ok(match rsp.error_for_status_to_client_error()? {
            StatusCode::OK => false,
            StatusCode::CREATED => true,
            _ => unreachable!("API cannot return anything but ok or created"),
        })
    }

    pub async fn get_store(&self, db_name: &str) -> Result<Store, ClientError> {
        let rsp = self
            .client
            .get(&format!("http://{}:{}/v1/get_db", self.path, self.port))
            .query(&["db_name", db_name])
            .send()
            .await?;
        rsp.error_for_status_to_client_error()?;
        let bytes = rsp.bytes().await?;
        Ok(Store::deser(bytes.as_ref())?)
    }

    pub async fn add_db_with_contents(
        &self,
        overwrite_existing: bool,
        name: &str,
        store: &Store,
    ) -> Result<bool, ClientError> {
        let store = store.ser()?;

        let rsp = self
            .client
            .put(&format!(
                "http://{}:{}/v1/add_db_with_content",
                self.path, self.port
            ))
            .query(&[
                (
                    "overwrite_existing",
                    if overwrite_existing { "true" } else { "false" },
                ),
                ("db_name", name),
            ])
            .body(store)
            .send()
            .await?;

        Ok(match rsp.error_for_status_to_client_error()? {
            StatusCode::OK => false,
            StatusCode::CREATED => true,
            _ => unreachable!("API cannot return anything but ok or created"),
        })
    }

    pub async fn add_entry_to_db(
        &self,
        database_name: &str,
        key: &str,
        value: &Value,
    ) -> Result<bool, ClientError> {
        let value = value.ser(None)?;
        let rsp = self
            .client
            .put(&format!("http://{}:{}/v1/add_kv", self.path, self.port))
            .query(&[("db_name", database_name), ("key", key)])
            .body(value)
            .send()
            .await?;

        Ok(match rsp.error_for_status_to_client_error()? {
            StatusCode::OK => false,
            StatusCode::CREATED => true,
            _ => unreachable!("API cannot return anything but ok or created"),
        })
    }

    pub async fn remove_entry_from_db(
        &self,
        database_name: &str,
        key: &str,
    ) -> Result<(), ClientError> {
        self.client
            .post(&format!("http://{}:{}/v1/rm_kv", self.path, self.port))
            .query(&[("db_name", database_name), ("key", key)])
            .send()
            .await?
            .error_for_status_to_client_error()?;
        Ok(())
    }

    pub async fn remove_db(&self, database_name: &str) -> Result<(), ClientError> {
        self.client
            .post(&format!("http://{}:{}/v1/rm_db", self.path, self.port))
            .query(&[("db_name", database_name)])
            .send()
            .await?
            .error_for_status_to_client_error()?;
        Ok(())
    }
}

trait ResponseExt {
    fn error_for_status_to_client_error(&self) -> Result<StatusCode, ClientError>;
}

impl ResponseExt for Response {
    fn error_for_status_to_client_error(&self) -> Result<StatusCode, ClientError> {
        let status = self.status();
        if status.is_success() {
            Ok(status)
        } else {
            Err(ClientError::HttpErrorCode(status))
        }
    }
}
