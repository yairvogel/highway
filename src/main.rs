use anyhow::Context;
use clap::{Parser, Subcommand};
use std::process::Command;
use url::Url;

mod k8s;
mod rule;
mod rule_parser;

use crate::k8s::*;
use crate::rule::Request;
use crate::rule_parser::parse_rule;

struct NamedRoute {
    name: String,
    route: Route,
}

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    // /// Name of the person to greet
    // #[arg(short, long)]
    // name: String,
    //
    // /// Number of times to greet
    // #[arg(short, long, default_value_t = 1)]
    // count: u8,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// lists all configured routes
    List,
    Match {
        url: String,
        #[arg(short = 'X', long, default_value = "GET")]
        method: String,
    },
}

fn build_routes() -> anyhow::Result<Vec<NamedRoute>> {
    let yamls = Command::new("kubectl")
        .args(&["get", "ingressroutes", "-o=yaml", "--all-namespaces"])
        .output()
        .context("failed executing kubectl")?;

    let ingress_routes: ResourceList<IngressRoute> = serde_yaml::from_slice(&yamls.stdout)
        .context("deserialization error")
        .context("failed deserializing")?;

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
    Ok(routes)
}

fn main() -> anyhow::Result<()> {
    let routes = build_routes()?;

    let args = Args::parse();
    match args.command {
        Commands::List => list_routes(&routes),
        Commands::Match { url, method } => match_routes(&routes, url, method)?,
    }

    Ok(())
}

fn match_routes(routes: &[NamedRoute], url: String, method: String) -> anyhow::Result<()> {
    let url = match Url::parse(&url) {
        Ok(url) => url,
        Err(e) => {
            eprintln!("provided url {url} is invalid: {e}");
            anyhow::bail!("failed to parse url");
        }
    };

    let request = Request { url, method };
    for route in routes.iter() {
        if parse_rule(&route.route.match_rule)?.match_request(&request) {
            println!(
                "{}: {}",
                route.name,
                route
                    .route
                    .services
                    .as_ref()
                    .unwrap()
                    .iter()
                    .next()
                    .unwrap()
                    .name
                    .as_str()
            );
            break;
        }
    }

    Ok(())
}

fn list_routes(routes: &[NamedRoute]) {
    for route in routes.iter() {
        println!(
            "{}: {}",
            route.name,
            parse_rule(&route.route.match_rule).unwrap()
        );
    }
}
