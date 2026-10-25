//! The HTTP cloud drivers end to end. None of these talk to the real services.
//!
//! * Snowflake: always, against an in-test mock of its REST API.
//! * BigQuery: the JWT / token / paging flow always runs against a mock; the data paths
//!   run against the goccy emulator when an endpoint is given:
//!   LIBRATABLE_TEST_BIGQUERY_ENDPOINT=http://127.0.0.1:19050
//!   (`docker run -p 127.0.0.1:19050:9050 ghcr.io/goccy/bigquery-emulator:latest
//!   --project=libratable-test --dataset=libratable_ds`)
//! * DynamoDB: against DynamoDB Local when an endpoint is given:
//!   LIBRATABLE_TEST_DYNAMODB_ENDPOINT=http://127.0.0.1:18000
//!   (`docker run -p 127.0.0.1:18000:8000 amazon/dynamodb-local:latest
//!   -jar DynamoDBLocal.jar -inMemory -sharedDb`)
//!
//! Only objects named `libratable_*` are created, and they are dropped even when a
//! check fails.

mod bigquery;
mod common;
mod dynamodb;
mod mock;
mod snowflake;
