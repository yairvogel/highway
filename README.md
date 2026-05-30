# highway

[![Tests](https://github.com/yairvogel/highway/actions/workflows/tests.yml/badge.svg)](https://github.com/yairvogel/highway/actions/workflows/tests.yml)

A small CLI for inspecting [Traefik](https://traefik.io/) routing in a Kubernetes
cluster. `highway` reads the `IngressRoute` resources from your current cluster
(via `kubectl`) and lets you:

- **list** every configured route, sorted by Traefik routing priority, and
- **match** a URL against those routes to see which one a request would hit, and
  which backend service it would be sent to.

It implements Traefik's [match-rule grammar](https://doc.traefik.io/traefik/reference/routing-configuration/http/routing/rules-and-priority/#rules)
(`Host`, `Path`, `PathPrefix`, `PathRegexp`, `Method`, combined with `&&`, `||`,
`!`, and parentheses), so matching mirrors how Traefik itself would route the
request.

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) (2024 edition / a recent stable
  toolchain) to build.
- [`kubectl`](https://kubernetes.io/docs/tasks/tools/) on your `PATH`, configured
  to talk to the cluster you want to inspect. `highway` shells out to
  `kubectl get ingressroutes -o=yaml --all-namespaces`.
- The cluster must have the Traefik `IngressRoute` CRD installed.

## Building

```sh
cargo build --release
```

The binary is produced at `target/release/highway`. You can also run it directly
during development with `cargo run -- <args>`.

## Usage

```sh
highway <COMMAND>
```

### `list`

List all routes discovered in the cluster, ordered by priority (highest first):

```sh
highway list
```

Each line shows the route name (the `IngressRoute` resource name) and its parsed
match rule, for example:

```
my-app: Host(`app.example.com`) && PathPrefix(`/api`)
dashboard: Host(`dashboard.example.com`)
```

### `match`

Simulate a request and print the first route that matches, along with the
service it routes to:

```sh
highway match <URL> [-X <METHOD>]
```

- `<URL>` — the full request URL, e.g. `https://app.example.com/api/users`.
- `-X`, `--method` — the HTTP method (defaults to `GET`).

Examples:

```sh
# Which route handles this request?
highway match https://app.example.com/api/users

# Match a POST request
highway match https://app.example.com/api/users -X POST
```

Output is the matched route name and its backend service:

```
my-app: my-app-service
```

Routes are evaluated in priority order (Traefik uses an explicit `priority` if
set, otherwise the length of the match rule), and the first match wins — the same
precedence Traefik applies.

## Development

Run the test suite:

```sh
cargo test
```

CI runs `cargo build` and `cargo test` on every push to `main` and on pull
requests; see [`.github/workflows/ci.yml`](.github/workflows/ci.yml).
