use anyhow::Context;
use std::process::Command;

mod k8s;
mod rule;
mod rule_parser;

use crate::k8s::*;
use crate::rule_parser::parse_rule;

struct NamedRoute {
    name: String,
    route: Route,
}

fn main() -> anyhow::Result<()> {
    let yamls = Command::new("kubectl")
        .args(&["get", "ingressroutes", "-o=yaml", "--all-namespaces"])
        .output()?;

    println!("read {} bytes", yamls.stdout.len());

    let ingress_routes: ResourceList<IngressRoute> =
        serde_yaml::from_slice(&yamls.stdout).context("deserialization error")?;

    let mut routes = Vec::with_capacity(ingress_routes.items.len());
    for ingress_route in ingress_routes.items {
        for route in ingress_route.spec.routes {
            routes.push(NamedRoute {
                route,
                name: ingress_route.metadata.name.clone(),
            })
        }
    }

    routes.sort_by_key(|r| -r.route.priority());

    for route in routes.iter() {
        println!(
            "{}: {}",
            route.name,
            parse_rule(&route.route.match_rule).unwrap()
        );
    }

    Ok(())
}
