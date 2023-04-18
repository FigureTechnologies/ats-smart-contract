use crate::error::Result;
use cosm_orc::orchestrator::{InstantiateResponse, QueryResponse};
use serde::Serialize;
use std::str;

pub(crate) trait SerializeExt {
    fn json_string(&self) -> Result<String>;
}

impl<T: ?Sized + Serialize> SerializeExt for T {
    fn json_string(&self) -> Result<String> {
        serde_json::to_string(&self).map_err(Into::into)
    }
}

pub(crate) trait CosmResponseExt {
    fn to_utf8_string(&self) -> Result<String>;
}

impl CosmResponseExt for InstantiateResponse {
    fn to_utf8_string(&self) -> Result<String> {
        match self.res.res.data {
            Some(ref bytes) => str::from_utf8(bytes)
                .map(|s| s.to_owned())
                .map_err(Into::into),
            None => Ok(String::new()),
        }
    }
}

impl CosmResponseExt for QueryResponse {
    fn to_utf8_string(&self) -> Result<String> {
        match self.res.data {
            Some(ref bytes) => str::from_utf8(bytes)
                .map(|s| s.to_owned())
                .map_err(Into::into),
            None => Ok(String::new()),
        }
    }
}
