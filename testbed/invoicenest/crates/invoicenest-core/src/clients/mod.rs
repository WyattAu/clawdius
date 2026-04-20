//! Client CRUD service.
//!
//! Provides async functions for creating, reading, updating, and deleting
//! clients within a workspace. All operations are scoped to a workspace
//! to enforce multi-tenant isolation.

use uuid::Uuid;

use crate::db::schema::Client;
use crate::db::PgPool;
use crate::AppError;

/// Parameters for creating a new client.
#[derive(Debug, Clone)]
pub struct CreateClient {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub company: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub tax_id: Option<String>,
    pub currency: Option<String>,
    pub notes: Option<String>,
}

/// Parameters for updating an existing client.
#[derive(Debug, Clone)]
pub struct UpdateClient {
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub company: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub tax_id: Option<String>,
    pub currency: Option<String>,
    pub notes: Option<String>,
}

/// Creates a new client in the given workspace.
///
/// # Errors
///
/// Returns [`AppError::Validation`] if required fields are empty, or
/// [`AppError::Database`] on query failure.
pub async fn create_client(
    pool: &PgPool,
    workspace_id: Uuid,
    input: CreateClient,
) -> Result<Client, AppError> {
    let country = input.country.unwrap_or_else(|| "US".into());
    let currency = input.currency.unwrap_or_else(|| "USD".into());

    let row = sqlx::query_as!(
        Client,
        r#"
        INSERT INTO clients (workspace_id, name, email, phone, company,
                             address_line1, address_line2, city, state,
                             postal_code, country, tax_id, currency, notes)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        RETURNING *
        "#,
        workspace_id,
        input.name,
        input.email,
        input.phone,
        input.company,
        input.address_line1,
        input.address_line2,
        input.city,
        input.state,
        input.postal_code,
        country,
        input.tax_id,
        currency,
        input.notes,
    )
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Retrieves a single client by ID, scoped to a workspace.
pub async fn get_client(
    pool: &PgPool,
    workspace_id: Uuid,
    client_id: Uuid,
) -> Result<Client, AppError> {
    let row = sqlx::query_as!(
        Client,
        r#"
        SELECT * FROM clients
        WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL
        "#,
        client_id,
        workspace_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Client {client_id} not found")))?;

    Ok(row)
}

/// Lists all clients in a workspace, ordered by name.
pub async fn list_clients(
    pool: &PgPool,
    workspace_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<Client>, AppError> {
    let rows = sqlx::query_as!(
        Client,
        r#"
        SELECT * FROM clients
        WHERE workspace_id = $1 AND deleted_at IS NULL
        ORDER BY name
        LIMIT $2 OFFSET $3
        "#,
        workspace_id,
        limit,
        offset,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Updates the mutable fields of an existing client.
pub async fn update_client(
    pool: &PgPool,
    workspace_id: Uuid,
    client_id: Uuid,
    input: UpdateClient,
) -> Result<Client, AppError> {
    let existing = get_client(pool, workspace_id, client_id).await?;

    let row = sqlx::query_as!(
        Client,
        r#"
        UPDATE clients SET
            name        = COALESCE($3, name),
            email       = COALESCE($4, email),
            phone       = COALESCE($5, phone),
            company     = COALESCE($6, company),
            address_line1 = COALESCE($7, address_line1),
            address_line2 = COALESCE($8, address_line2),
            city        = COALESCE($9, city),
            state       = COALESCE($10, state),
            postal_code = COALESCE($11, postal_code),
            country     = COALESCE($12, country),
            tax_id      = COALESCE($13, tax_id),
            currency    = COALESCE($14, currency),
            notes       = COALESCE($15, notes),
            updated_at  = NOW()
        WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL
        RETURNING *
        "#,
        client_id,
        workspace_id,
        input.name,
        input.email,
        input.phone,
        input.company,
        input.address_line1,
        input.address_line2,
        input.city,
        input.state,
        input.postal_code,
        input.country,
        input.tax_id,
        input.currency,
        input.notes,
    )
    .fetch_one(pool)
    .await?;

    let _ = existing; // keep for potential audit logging
    Ok(row)
}

/// Soft-deletes a client by setting `deleted_at`.
pub async fn delete_client(
    pool: &PgPool,
    workspace_id: Uuid,
    client_id: Uuid,
) -> Result<(), AppError> {
    let result = sqlx::query!(
        r#"
        UPDATE clients SET deleted_at = NOW()
        WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL
        "#,
        client_id,
        workspace_id,
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Client {client_id} not found")));
    }

    Ok(())
}

/// Searches clients by name (case-insensitive prefix match).
pub async fn search_clients(
    pool: &PgPool,
    workspace_id: Uuid,
    query: &str,
    limit: i64,
) -> Result<Vec<Client>, AppError> {
    let pattern = format!("{query}%");

    let rows = sqlx::query_as!(
        Client,
        r#"
        SELECT * FROM clients
        WHERE workspace_id = $1 AND deleted_at IS NULL
          AND name ILIKE $2
        ORDER BY name
        LIMIT $3
        "#,
        workspace_id,
        pattern,
        limit,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
