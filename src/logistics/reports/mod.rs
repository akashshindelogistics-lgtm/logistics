//! Basic operational reporting for an organisation: how hard the fleet is
//! working, how delivery is going, what each godown is holding, and how
//! dispatch volume has moved over the last two weeks.
//!
//! Everything here is derived — there are no new tables. [`OpsReport::for_org`]
//! loads the org's dispatches, vehicles and godowns through their existing
//! list methods and folds them into the figures below, the same way
//! [`crate::logistics::billing::invoice::Invoice::customer_summary`] works off
//! the full invoice list.

use crate::logistics::dispatch::dispatch::{DispatchOrder, DispatchStatus};
use crate::logistics::godown::godown::Godown;
use crate::logistics::vehicle::vehicle::Vehicle;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// A dispatch counts as "on time" when it reaches `DELIVERED` within this many
/// hours of being created. The system has no per-order promised date yet, so
/// this is one fleet-wide target; when a real SLA / promised-date field lands,
/// swap this constant for a per-dispatch comparison.
const ON_TIME_TARGET_HOURS: f64 = 72.0;
/// How many days the dispatch-volume series covers (including today).
const VOLUME_WINDOW_DAYS: i64 = 14;
/// "Recently dispatched" units look back this many days.
const RECENT_WINDOW_DAYS: i64 = 30;

const SECONDS_PER_DAY: i64 = 86_400;

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Round to one decimal place.
fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// `z` days from the Unix epoch to a `(year, month, day)` (Howard Hinnant's
/// `civil_from_days`). Duplicated from `billing::invoice` / `vehicle::document`
/// on the same "~15 lines, not worth a shared module" basis noted there.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn iso_date_from_epoch_day(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Share of the fleet currently out on a trip.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct VehicleUtilization {
    pub total_vehicles: i64,
    /// Distinct vehicles this org owns that have a dispatch in a non-terminal
    /// status (`PENDING` / `CONFIRMED` / `LOADED` / `IN_TRANSIT`).
    pub vehicles_on_active_trip: i64,
    /// `vehicles_on_active_trip / total_vehicles * 100`, one decimal. `0.0`
    /// when the org has no vehicles.
    pub utilization_percent: f64,
}

/// How delivery is going across every dispatch the org has ever created.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DeliveryPerformance {
    pub delivered_count: i64,
    pub returned_count: i64,
    /// Mean hours from a dispatch's creation to its `DELIVERED` event, over
    /// every delivered dispatch. `None` when nothing has been delivered yet.
    pub avg_hours_to_deliver: Option<f64>,
    /// Share of delivered dispatches that reached `DELIVERED` within
    /// `ON_TIME_TARGET_HOURS` (72h). `None` when nothing has been delivered.
    pub on_time_rate_percent: Option<f64>,
}

/// What one godown is holding right now.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GodownInventory {
    pub godown_id: Uuid,
    pub godown_name: String,
    /// Σ(quantity) across the godown's stock rows.
    pub units_on_hand: i64,
    pub distinct_items: i64,
    /// Σ(volume_in_size × quantity) as a percentage of `max_capacity`, one
    /// decimal. `None` when the godown has no cap set.
    pub capacity_used_percent: Option<f64>,
}

/// One day's dispatch count.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DispatchVolumePoint {
    /// `YYYY-MM-DD`, UTC.
    pub date: String,
    pub count: i64,
}

/// The whole report for one organisation.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OpsReport {
    pub vehicle_utilization: VehicleUtilization,
    pub delivery_performance: DeliveryPerformance,
    /// Σ(line-item quantity) over dispatches created in the last
    /// `RECENT_WINDOW_DAYS` (30) days, any status. Dispatches are not attributed
    /// to a source godown, so this is an org-wide figure, not per-godown.
    pub units_dispatched_recently: i64,
    pub godown_inventory: Vec<GodownInventory>,
    /// One point per day for the last `VOLUME_WINDOW_DAYS` (14) days, oldest
    /// first. Days with no dispatches are present with `count: 0`.
    pub dispatch_volume: Vec<DispatchVolumePoint>,
}

impl OpsReport {
    pub fn for_org(org_id: Uuid) -> Result<Self, Box<dyn Error>> {
        let vehicles = Vehicle::list_by_org(org_id)?;
        let dispatches = DispatchOrder::list_by_org(org_id)?;
        let godowns = Godown::list_by_org(org_id)?;
        let now = now_unix();

        Ok(Self {
            vehicle_utilization: vehicle_utilization(&vehicles, &dispatches),
            delivery_performance: delivery_performance(&dispatches),
            units_dispatched_recently: units_dispatched_since(
                &dispatches,
                now - RECENT_WINDOW_DAYS * SECONDS_PER_DAY,
            ),
            godown_inventory: godowns.iter().map(godown_inventory).collect(),
            dispatch_volume: dispatch_volume(&dispatches, now),
        })
    }
}

fn vehicle_utilization(vehicles: &[Vehicle], dispatches: &[DispatchOrder]) -> VehicleUtilization {
    let active_regs: HashSet<&str> = dispatches
        .iter()
        .filter(|d| !d.status.is_terminal())
        .map(|d| d.vehicle_registration_number.as_str())
        .collect();

    let total_vehicles = vehicles.len() as i64;
    let vehicles_on_active_trip = vehicles
        .iter()
        .filter(|v| active_regs.contains(v.registration_number.as_str()))
        .count() as i64;

    let utilization_percent = if total_vehicles == 0 {
        0.0
    } else {
        round1(vehicles_on_active_trip as f64 / total_vehicles as f64 * 100.0)
    };

    VehicleUtilization {
        total_vehicles,
        vehicles_on_active_trip,
        utilization_percent,
    }
}

fn delivery_performance(dispatches: &[DispatchOrder]) -> DeliveryPerformance {
    let returned_count = dispatches
        .iter()
        .filter(|d| d.status == DispatchStatus::Returned)
        .count() as i64;

    // Hours from creation to the DELIVERED event, for every delivered dispatch.
    let delivery_hours: Vec<f64> = dispatches
        .iter()
        .filter(|d| d.status == DispatchStatus::Delivered)
        .filter_map(|d| {
            d.status_history
                .iter()
                .find(|e| e.status == DispatchStatus::Delivered)
                .map(|e| (e.changed_at - d.dispatched_at).max(0) as f64 / 3600.0)
        })
        .collect();

    let delivered_count = delivery_hours.len() as i64;
    let (avg_hours_to_deliver, on_time_rate_percent) = if delivery_hours.is_empty() {
        (None, None)
    } else {
        let avg = delivery_hours.iter().sum::<f64>() / delivery_hours.len() as f64;
        let on_time = delivery_hours
            .iter()
            .filter(|h| **h <= ON_TIME_TARGET_HOURS)
            .count() as f64;
        (
            Some(round1(avg)),
            Some(round1(on_time / delivery_hours.len() as f64 * 100.0)),
        )
    };

    DeliveryPerformance {
        delivered_count,
        returned_count,
        avg_hours_to_deliver,
        on_time_rate_percent,
    }
}

fn units_dispatched_since(dispatches: &[DispatchOrder], since: i64) -> i64 {
    dispatches
        .iter()
        .filter(|d| d.dispatched_at >= since)
        .flat_map(|d| d.line_items.iter())
        .map(|li| li.quantity)
        .sum()
}

fn godown_inventory(godown: &Godown) -> GodownInventory {
    let units_on_hand = godown.stock.iter().map(|s| s.quantity).sum();
    let used_volume: i64 = godown
        .stock
        .iter()
        .map(|s| s.volume_in_size * s.quantity)
        .sum();
    let capacity_used_percent = godown.max_capacity.and_then(|cap| {
        (cap > 0).then(|| round1(used_volume as f64 / cap as f64 * 100.0))
    });

    GodownInventory {
        godown_id: godown.id,
        godown_name: godown.name.clone(),
        units_on_hand,
        distinct_items: godown.stock.len() as i64,
        capacity_used_percent,
    }
}

fn dispatch_volume(dispatches: &[DispatchOrder], now: i64) -> Vec<DispatchVolumePoint> {
    let today = now.div_euclid(SECONDS_PER_DAY);

    let mut counts: HashMap<i64, i64> = HashMap::new();
    for d in dispatches {
        let day = d.dispatched_at.div_euclid(SECONDS_PER_DAY);
        if (0..VOLUME_WINDOW_DAYS).contains(&(today - day)) {
            *counts.entry(day).or_insert(0) += 1;
        }
    }

    (0..VOLUME_WINDOW_DAYS)
        .rev()
        .map(|offset| {
            let day = today - offset;
            DispatchVolumePoint {
                date: iso_date_from_epoch_day(day),
                count: counts.get(&day).copied().unwrap_or(0),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
