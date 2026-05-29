use anyhow::Context;
use std::{fmt::Display, process::Command};

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

fn main() -> anyhow::Result<()> {
    let yamls = Command::new("kubectl")
        .args(&["get", "ingressroutes", "-o=yaml", "--all-namespaces"])
        .output()?;

    println!("read {} bytes", yamls.stdout.len());

    let ingress_routes: ResourceList<IngressRoute> =
        serde_yaml::from_slice(&yamls.stdout).context("deserialization error")?;

    let mut routes: Vec<Route> = ingress_routes
        .items
        .into_iter()
        .flat_map(|i| i.spec.routes)
        .collect();

    routes.sort_by_key(|r| -r.priority());

    for route in routes.iter() {
        println!("{route}");
    }

    Ok(())
}
