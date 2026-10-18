use super::*;
use crate::import::{self, ImportedItem};

const OPENAPI: &str = r##"
openapi: 3.0.3
info:
  title: Pet Store
servers:
  - url: https://{region}.pets.test/v1/
    variables:
      region:
        default: eu
tags:
  - name: pets
  - name: owners
security:
  - bearer: []
components:
  securitySchemes:
    bearer:
      type: http
      scheme: bearer
    key:
      type: apiKey
      in: header
      name: X-Api-Key
  parameters:
    PetId:
      name: petId
      in: path
      required: true
      schema:
        type: integer
  schemas:
    Pet:
      type: object
      properties:
        name:
          type: string
        born:
          type: string
          format: date
paths:
  /pets:
    get:
      tags: [pets]
      summary: List pets
      parameters:
        - name: limit
          in: query
          required: true
          schema:
            type: integer
            default: 20
        - name: cursor
          in: query
          schema:
            type: string
      responses:
        "200":
          description: ok
          content:
            application/json: {}
    post:
      tags: [pets]
      operationId: createPet
      requestBody:
        content:
          application/json:
            schema:
              $ref: "#/components/schemas/Pet"
      responses:
        "201":
          description: created
  /pets/{petId}:
    parameters:
      - $ref: "#/components/parameters/PetId"
    delete:
      tags: [pets]
      security:
        - key: []
      responses:
        "204":
          description: gone
  /health:
    get:
      security: []
      requestBody:
        content:
          application/json:
            examples:
              ok:
                value: { "deep": true }
      responses: {}
"##;

const SWAGGER: &str = r##"{
  "swagger": "2.0",
  "info": { "title": "Legacy" },
  "host": "legacy.test",
  "basePath": "/api",
  "schemes": ["http"],
  "produces": ["application/json"],
  "securityDefinitions": { "key": { "type": "apiKey", "in": "query", "name": "api_key" } },
  "security": [{ "key": [] }],
  "definitions": { "Item": { "type": "object", "properties": { "qty": { "type": "integer" } } } },
  "paths": {
    "/items": {
      "post": {
        "summary": "Add item",
        "parameters": [
          { "name": "body", "in": "body", "schema": { "$ref": "#/definitions/Item" } },
          { "name": "X-Request-Id", "in": "header", "type": "string", "default": "abc" }
        ]
      }
    }
  }
}"##;

fn folder(items: &[ImportedItem], index: usize) -> (&str, &[ImportedItem]) {
    match &items[index] {
        ImportedItem::Folder { name, items, .. } => (name, items),
        ImportedItem::Request(_) => panic!("expected folder"),
    }
}

fn request(items: &[ImportedItem], index: usize) -> &Request {
    match &items[index] {
        ImportedItem::Request(request) => request,
        ImportedItem::Folder { .. } => panic!("expected request"),
    }
}

fn header_value<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|h| h.name == name)
        .map(|h| h.value.as_str())
}

#[test]
fn openapi_groups_by_tag_and_builds_environment() {
    let collection = import::parse(OPENAPI).unwrap();
    assert_eq!(collection.format, ImportFormat::OpenApi);
    assert_eq!(collection.name, "Pet Store");

    // "owners" has no operations, so only "pets" plus the untagged request.
    assert_eq!(collection.items.len(), 2);
    let (name, pets) = folder(&collection.items, 0);
    assert_eq!(name, "pets");
    assert_eq!(pets.len(), 3);

    let env = &collection.environments[0];
    assert_eq!(env.variables["baseUrl"], "https://eu.pets.test/v1");
    assert_eq!(env.secrets, ["token", "apiKey"]);
}

#[test]
fn openapi_operations_get_params_bodies_and_auth() {
    let collection = import::parse(OPENAPI).unwrap();
    let (_, pets) = folder(&collection.items, 0);

    let list = request(pets, 0);
    assert_eq!(list.name, "List pets");
    assert_eq!(list.url, "{{baseUrl}}/pets?limit=20");
    assert_eq!(header_value(list, "Accept"), Some("application/json"));
    assert_eq!(
        header_value(list, "Authorization"),
        Some("Bearer {{token}}")
    );

    let create = request(pets, 1);
    assert_eq!(create.name, "createPet");
    let body: Value = serde_json::from_str(&create.body).unwrap();
    assert_eq!(body["born"], "2026-01-01");
    assert_eq!(
        header_value(create, "Content-Type"),
        Some("application/json")
    );

    let delete = request(pets, 2);
    assert_eq!(delete.name, "DELETE /pets/{petId}");
    assert_eq!(delete.url, "{{baseUrl}}/pets/{{petId}}");
    assert_eq!(header_value(delete, "X-Api-Key"), Some("{{apiKey}}"));

    let health = request(&collection.items, 1);
    assert!(header_value(health, "Authorization").is_none());
    let body: Value = serde_json::from_str(&health.body).unwrap();
    assert_eq!(body["deep"], true);
}

#[test]
fn swagger_uses_host_body_params_and_query_keys() {
    let collection = import::parse(SWAGGER).unwrap();
    assert_eq!(collection.format, ImportFormat::Swagger);
    assert_eq!(
        collection.environments[0].variables["baseUrl"],
        "http://legacy.test/api"
    );

    let add = request(&collection.items, 0);
    assert_eq!(add.url, "{{baseUrl}}/items?api_key={{apiKey}}");
    assert_eq!(add.body, "{\n  \"qty\": 0\n}");
    assert_eq!(header_value(add, "X-Request-Id"), Some("abc"));
    assert_eq!(header_value(add, "Accept"), Some("application/json"));
}
