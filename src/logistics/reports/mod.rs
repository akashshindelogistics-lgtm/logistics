//! Basic operational reporting for an organisation: how hard the fleet is
//! working, how delivery is going, what each godown is holding, and how
//! dispatch volume has moved over the last two weeks.
//!
//! Everything here is derived — there are no new tables. [`OpsReport::for_org`]
//! loads the org's dispatches, vehicles and godowns through their existing
//! list methods and folds them into the figures below, the same way
//! [`crate::logistics::billing::invoice::Invoice::customer_summary`] works off
//! the full invoice list.

use crate::logistics::billing::invoice::Invoice;
use crate::logistics::dispatch::dispatch::{DispatchOrder, DispatchStatus, VehicleSource};
use crate::logistics::godown::godown::Godown;
use crate::logistics::vehicle::vehicle::Vehicle;
use crate::logistics::vendor::hire::{HireStatus, VehicleHire};
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

/// Total units held under one category, within a single godown.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CategoryUnits {
    pub category: String,
    pub units: i64,
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
    /// One entry per distinct category held in this godown, largest units
    /// first, ties broken alphabetically.
    pub category_breakdown: Vec<CategoryUnits>,
}

/// One day's dispatch count.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DispatchVolumePoint {
    /// `YYYY-MM-DD`, UTC.
    pub date: String,
    pub count: i64,
}

/// What one vendor has been paid, and is still owed, across its hires.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct VendorSpend {
    pub vendor_id: Uuid,
    pub vendor_name: String,
    /// Hires with a truck and rate assigned (`CONFIRMED` or `RELEASED`).
    pub hires: i64,
    /// Σ agreed hire cost over those hires.
    pub hire_cost: i64,
    pub paid: i64,
    pub outstanding: i64,
}

/// Freight invoiced against the dispatches one hire carried, minus what the
/// truck cost. A trip's hire covers all its stops, so margin is per hire.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct HireMargin {
    pub hire_id: Uuid,
    pub vendor_name: String,
    pub registration_number: Option<String>,
    /// `Some` when the hire was for a multi-stop trip.
    pub trip_id: Option<Uuid>,
    /// Dispatches on this hire that weren't cancelled.
    pub dispatches: i64,
    /// How many of those have an invoice yet. Until every one does, `margin`
    /// understates what the hire will earn.
    pub invoiced_dispatches: i64,
    /// Σ invoice amount over those dispatches.
    pub invoiced: i64,
    pub hire_cost: i64,
    /// `invoiced - hire_cost`.
    pub margin: i64,
}

/// How much the org leans on hired trucks, and what they cost.
#[derive(Debug, Clone, Default, Serialize, Deserialize, utoipa::ToSchema)]
pub struct HiredTransport {
    /// Non-cancelled dispatches on the org's own vehicles.
    pub own_dispatches: i64,
    /// Non-cancelled dispatches on hired trucks (assigned or still awaiting).
    pub hired_dispatches: i64,
    /// `hired / (own + hired) * 100`, one decimal. `None` with no dispatches.
    pub hired_share_percent: Option<f64>,
    /// Σ agreed hire cost over hires with a truck assigned.
    pub hire_cost_total: i64,
    pub paid_to_vendors: i64,
    pub outstanding_to_vendors: i64,
    /// Hires still waiting for the vendor's truck.
    pub awaiting_truck: i64,
    /// One row per vendor with an assigned hire, most spent first.
    pub vendors: Vec<VendorSpend>,
    /// One row per assigned hire, newest first.
    pub hire_margins: Vec<HireMargin>,
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
    /// Own vs hired trucks, vendor spend and margin on hires.
    #[serde(default)]
    pub hired_transport: HiredTransport,
}

impl OpsReport {
    pub fn for_org(org_id: Uuid) -> Result<Self, Box<dyn Error>> {
        let vehicles = Vehicle::list_by_org(org_id)?;
        let dispatches = DispatchOrder::list_by_org(org_id)?;
        let godowns = Godown::list_by_org(org_id)?;
        let hires = VehicleHire::list_by_org(org_id, None)?;
        let invoices = Invoice::list_by_org(org_id)?;
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
            hired_transport: hired_transport(&dispatches, &hires, &invoices),
        })
    }
}

fn hired_transport(
    dispatches: &[DispatchOrder],
    hires: &[VehicleHire],
    invoices: &[Invoice],
) -> HiredTransport {
    let live: Vec<&DispatchOrder> = dispatches
        .iter()
        .filter(|d| d.status != DispatchStatus::Cancelled)
        .collect();
    let hired_dispatches = live.iter().filter(|d| d.vehicle_source == VehicleSource::Hired).count() as i64;
    let own_dispatches = live.len() as i64 - hired_dispatches;
    let hired_share_percent = (!live.is_empty())
        .then(|| round1(hired_dispatches as f64 / live.len() as f64 * 100.0));

    let invoice_by_dispatch: HashMap<Uuid, i64> =
        invoices.iter().map(|i| (i.dispatch_id, i.amount)).collect();

    // Hires with a truck and rate; a REQUESTED or CANCELLED hire has no cost.
    let assigned: Vec<&VehicleHire> = hires
        .iter()
        .filter(|h| matches!(h.status, HireStatus::Confirmed | HireStatus::Released))
        .collect();

    let mut by_vendor: HashMap<Uuid, VendorSpend> = HashMap::new();
    for h in &assigned {
        let v = by_vendor.entry(h.vendor_id).or_insert_with(|| VendorSpend {
            vendor_id: h.vendor_id,
            vendor_name: h.vendor_name.clone(),
            hires: 0,
            hire_cost: 0,
            paid: 0,
            outstanding: 0,
        });
        v.hires += 1;
        v.hire_cost += h.freight_amount.unwrap_or(0);
        v.paid += h.total_paid;
        v.outstanding += h.balance_due.unwrap_or(0);
    }
    let mut vendors: Vec<VendorSpend> = by_vendor.into_values().collect();
    vendors.sort_by(|a, b| b.hire_cost.cmp(&a.hire_cost).then_with(|| a.vendor_name.cmp(&b.vendor_name)));

    let mut hire_margins: Vec<(i64, HireMargin)> = assigned
        .iter()
        .map(|h| {
            let on_hire: Vec<&&DispatchOrder> =
                live.iter().filter(|d| d.hire_id == Some(h.id)).collect();
            let amounts: Vec<i64> = on_hire
                .iter()
                .filter_map(|d| invoice_by_dispatch.get(&d.id).copied())
                .collect();
            let invoiced: i64 = amounts.iter().sum();
            let hire_cost = h.freight_amount.unwrap_or(0);
            (
                h.requested_at,
                HireMargin {
                    hire_id: h.id,
                    vendor_name: h.vendor_name.clone(),
                    registration_number: h.registration_number.clone(),
                    trip_id: h.trip_id,
                    dispatches: on_hire.len() as i64,
                    invoiced_dispatches: amounts.len() as i64,
                    invoiced,
                    hire_cost,
                    margin: invoiced - hire_cost,
                },
            )
        })
        .collect();
    hire_margins.sort_by(|a, b| b.0.cmp(&a.0));

    HiredTransport {
        own_dispatches,
        hired_dispatches,
        hired_share_percent,
        hire_cost_total: assigned.iter().map(|h| h.freight_amount.unwrap_or(0)).sum(),
        paid_to_vendors: assigned.iter().map(|h| h.total_paid).sum(),
        outstanding_to_vendors: assigned.iter().map(|h| h.balance_due.unwrap_or(0)).sum(),
        awaiting_truck: hires.iter().filter(|h| h.status == HireStatus::Requested).count() as i64,
        vendors,
        hire_margins: hire_margins.into_iter().map(|(_, m)| m).collect(),
    }
}

fn vehicle_utilization(vehicles: &[Vehicle], dispatches: &[DispatchOrder]) -> VehicleUtilization {
    let active_regs: HashSet<&str> = dispatches
        .iter()
        // Hired trucks aren't part of the fleet being measured.
        .filter(|d| !d.status.is_terminal() && d.vehicle_source == VehicleSource::Own)
        .filter_map(|d| d.vehicle_registration_number.as_deref())
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

    let mut by_category: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for s in &godown.stock {
        *by_category.entry(s.category.clone()).or_insert(0) += s.quantity;
    }
    let mut category_breakdown: Vec<CategoryUnits> = by_category
        .into_iter()
        .map(|(category, units)| CategoryUnits { category, units })
        .collect();
    category_breakdown
        .sort_by(|a, b| b.units.cmp(&a.units).then_with(|| a.category.cmp(&b.category)));

    GodownInventory {
        godown_id: godown.id,
        godown_name: godown.name.clone(),
        units_on_hand,
        distinct_items: godown.stock.len() as i64,
        capacity_used_percent,
        category_breakdown,
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
