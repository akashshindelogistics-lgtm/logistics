use crate::logistics::db::connection::DbConnection;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use uuid::Uuid;

fn ensure_dispatches_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
    conn.exec_drop(
        "CREATE TABLE IF NOT EXISTS Dispatches (
            id VARCHAR(36) PRIMARY KEY,
            org_id VARCHAR(36) NOT NULL,
            customer_id VARCHAR(36) NOT NULL,
            vehicle_registration_number VARCHAR(255) NOT NULL,
            stock_description VARCHAR(255) NOT NULL,
            quantity BIGINT NOT NULL,
            status VARCHAR(50) NOT NULL,
            dispatched_at BIGINT NOT NULL
        )",
        (),
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DispatchOrder {
    pub id: Uuid,
    pub org_id: Uuid,
    pub customer_id: Uuid,
    pub vehicle_registration_number: String,
    pub stock_description: String,
    pub quantity: i64,
    pub status: String,
    pub dispatched_at: i64,
}

impl DispatchOrder {
    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let db_connection = DbConnection::new("localhost", 3306, "logistics", "root", "password");
        let mut conn = db_connection.get_connection()?;
        ensure_dispatches_table(&mut conn)?;

        conn.exec_drop(
            "INSERT INTO Dispatches (id, org_id, customer_id, vehicle_registration_number, stock_description, quantity, status, dispatched_at)
             VALUES (:id, :org_id, :customer_id, :vehicle_registration_number, :stock_description, :quantity, :status, :dispatched_at)",
            params! {
                "id" => self.id.to_string(),
                "org_id" => self.org_id.to_string(),
                "customer_id" => self.customer_id.to_string(),
                "vehicle_registration_number" => &self.vehicle_registration_number,
                "stock_description" => &self.stock_description,
                "quantity" => self.quantity,
                "status" => &self.status,
                "dispatched_at" => self.dispatched_at,
            },
        )?;

        Ok(())
    }

    pub fn list_all() -> Result<Vec<Self>, Box<dyn Error>> {
        let db_connection = DbConnection::new("localhost", 3306, "logistics", "root", "password");
        let mut conn = db_connection.get_connection()?;
        ensure_dispatches_table(&mut conn)?;

        let rows: Vec<(String, String, String, String, String, i64, String, i64)> = conn.exec_map(
            "SELECT id, org_id, customer_id, vehicle_registration_number, stock_description, quantity, status, dispatched_at FROM Dispatches",
            (),
            |(id, org_id, customer_id, vehicle_reg, stock_desc, qty, status, dispatched_at)| {
                (id, org_id, customer_id, vehicle_reg, stock_desc, qty, status, dispatched_at)
            },
        )?;

        Ok(Self::map_rows(rows))
    }

    pub fn list_by_org(org_id: Uuid) -> Result<Vec<Self>, Box<dyn Error>> {
        let db_connection = DbConnection::new("localhost", 3306, "logistics", "root", "password");
        let mut conn = db_connection.get_connection()?;
        ensure_dispatches_table(&mut conn)?;

        let rows: Vec<(String, String, String, String, String, i64, String, i64)> = conn.exec_map(
            "SELECT id, org_id, customer_id, vehicle_registration_number, stock_description, quantity, status, dispatched_at FROM Dispatches WHERE org_id = :org_id",
            params! { "org_id" => org_id.to_string() },
            |(id, org_id, customer_id, vehicle_reg, stock_desc, qty, status, dispatched_at)| {
                (id, org_id, customer_id, vehicle_reg, stock_desc, qty, status, dispatched_at)
            },
        )?;

        Ok(Self::map_rows(rows))
    }

    fn map_rows(rows: Vec<(String, String, String, String, String, i64, String, i64)>) -> Vec<Self> {
        rows.into_iter()
            .map(|(id, org_id, customer_id, vehicle_reg, stock_desc, qty, status, dispatched_at)| {
                DispatchOrder {
                    id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
                    org_id: Uuid::parse_str(&org_id).unwrap_or_else(|_| Uuid::new_v4()),
                    customer_id: Uuid::parse_str(&customer_id).unwrap_or_else(|_| Uuid::new_v4()),
                    vehicle_registration_number: vehicle_reg,
                    stock_description: stock_desc,
                    quantity: qty,
                    status,
                    dispatched_at,
                }
            })
            .collect()
    }
}
