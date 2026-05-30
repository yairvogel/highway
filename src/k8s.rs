#![allow(dead_code)]

use std::fmt::Display;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct IngressRoute {
    pub metadata: Metadata,
    pub spec: IngressRouteSpec,
}

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct IngressRouteSpec {
    pub routes: Vec<Route>,
}

#[derive(Debug, Deserialize)]
pub struct Route {
    #[serde(rename = "match")]
    pub match_rule: String,
    #[serde(rename = "priority")]
    pub priority_override: Option<i64>,
    pub services: Option<Vec<Service>>,
}

impl Route {
    pub fn priority(&self) -> i64 {
        self.priority_override
            .unwrap_or(self.match_rule.len() as i64)
    }
}

impl Display for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.match_rule, self.priority())?;
        if let Some(services) = &self.services {
            for service in services {
                writeln!(f, "")?;
                write!(f, "\t- {}", service.name)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct Service {
    pub name: String,
    pub port: Option<String>,
    pub weight: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ResourceList<T> {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub items: Vec<T>,
}
