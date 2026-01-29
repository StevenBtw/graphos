//! Query language parsers.
//!
//! Each parser turns query text into Grafeo's internal representation.
//! Enable what you need via feature flags - only GQL is on by default.
//!
//! | Module | Language | Standard | Feature |
//! | ------ | -------- | -------- | ------- |
//! | [`gql`] | GQL | ISO/IEC 39075:2024 | `gql` (default) |
//! | [`cypher`] | Cypher | openCypher 9.0 | `cypher` |
//! | [`sparql`] | SPARQL | W3C SPARQL 1.1 | `sparql` |
//! | [`gremlin`] | Gremlin | Apache TinkerPop | `gremlin` |
//! | [`graphql`] | GraphQL | June 2018 spec | `graphql` |

#[cfg(feature = "gql")]
pub mod gql;

#[cfg(feature = "cypher")]
pub mod cypher;

#[cfg(feature = "sparql")]
pub mod sparql;

#[cfg(feature = "gremlin")]
pub mod gremlin;

#[cfg(feature = "graphql")]
pub mod graphql;
