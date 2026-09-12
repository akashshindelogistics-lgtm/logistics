export interface Location {
  latitude: number;
  longitude: number;
  timestamp: number;
  address?: string;
}

// Mirrors Unit in src/logistics/vehicle/vehicle.rs.
export type Unit = 'MetricTon' | 'Kg' | 'Litre' | 'Box' | 'Pallet' | 'Piece';
export const UNITS: Unit[] = ['MetricTon', 'Kg', 'Litre', 'Box', 'Pallet', 'Piece'];

export interface Vehicle {
  registration_number: string;
  capacity: number;
  unit: Unit;
  location?: Location;
  assigned_driver_id?: string | null;
  /**
   * Per-vehicle device credential. A GPS tracker POSTs coordinates to
   * `/api/track/<tracker_key>` with no login. Rotate it if a device is lost.
   * Always present on responses from the server; optional here only so the
   * many partial test fixtures don't all have to spell it out.
   */
  tracker_key?: string;
}

export interface Driver {
  id: string;
  org_id: string;
  name: string;
  license_number: string;
  phone: string;
  is_active: boolean;
}

export interface Stock {
  volume_in_size: number;
  quantity: number;
  description: string;
}

export interface Godown {
  id: string;
  org_id: string;
  name: string;
  address: string;
  /** Optional cap on total stored volume (Σ volume_in_size × quantity). */
  max_capacity?: number | null;
  location?: Location;
  stock: Stock[];
}

// Mirrors StockTransfer in src/logistics/godown/transfer.rs — one recorded
// move of a stock item between two godowns of the same organization.
export interface StockTransfer {
  id: string;
  org_id: string;
  from_godown_id: string;
  to_godown_id: string;
  description: string;
  quantity: number;
  volume_in_size: number;
  transferred_at: number;
}

// Mirrors ComplianceDocType / ComplianceStatus / VehicleDocument in
// src/logistics/vehicle/document.rs. `days_until_expiry` and `status` are
// computed by the server on every read.
export type ComplianceDocType =
  | 'Insurance'
  | 'RegistrationCertificate'
  | 'Permit'
  | 'PollutionCertificate'
  | 'FitnessCertificate';

export type ComplianceStatus = 'Valid' | 'ExpiringSoon' | 'Expired';

export interface VehicleDocument {
  id: string;
  org_id: string;
  vehicle_registration: string;
  doc_type: ComplianceDocType;
  document_number: string;
  issued_on: string | null;
  expires_on: string;
  notes: string | null;
  days_until_expiry: number;
  status: ComplianceStatus;
}

export interface Organization {
  id: string;
  name: string;
  address: string;
  vehicles: Vehicle[];
  godowns: Godown[];
  location?: Location;
}

export interface Customer {
  id: string;
  org_id: string;
  name: string;
  address: string;
  location?: Location;
  phone?: string | null;
  email?: string | null;
}

// Mirrors src/logistics/notification/notification.rs.
export type NotificationEvent = 'DISPATCH_CREATED' | 'DISPATCH_DELIVERED';
export type NotificationChannel = 'SMS' | 'EMAIL';
export type NotificationStatus = 'QUEUED' | 'SKIPPED';

export interface Notification {
  id: string;
  org_id: string;
  dispatch_id: string;
  event: NotificationEvent;
  channel: NotificationChannel;
  recipient_kind: string;
  recipient: string;
  body: string;
  status: NotificationStatus;
  created_at: number;
}

// Mirrors DispatchStatus in src/logistics/dispatch/dispatch.rs. Keep in sync
// if the backend state machine changes.
export type DispatchStatus =
  | 'PENDING'
  | 'CONFIRMED'
  | 'LOADED'
  | 'IN_TRANSIT'
  | 'DELIVERED'
  | 'RETURNED'
  | 'CANCELLED';

export interface DispatchStatusEvent {
  status: DispatchStatus;
  changed_at: number;
}

export interface ProofOfDelivery {
  receiver_name: string;
  signature_or_photo_url: string;
  delivered_at: number;
}

// One stock line on a dispatch. Mirrors DispatchLineItem in
// src/logistics/dispatch/dispatch.rs.
export interface DispatchLineItem {
  stock_description: string;
  quantity: number;
  volume_in_size: number;
}

export interface DispatchOrder {
  id: string;
  org_id: string;
  customer_id: string;
  vehicle_registration_number: string;
  line_items: DispatchLineItem[];
  status: DispatchStatus;
  dispatched_at: number;
  status_history: DispatchStatusEvent[];
  proof_of_delivery: ProofOfDelivery | null;
  /** Set when this dispatch is one stop on a multi-stop trip. */
  trip_id?: string | null;
  stop_sequence?: number | null;
}

// Mirrors src/logistics/dispatch/trip.rs.
export type TripStatus = 'PLANNED' | 'IN_PROGRESS' | 'COMPLETED';

export interface Trip {
  id: string;
  org_id: string;
  vehicle_registration_number: string;
  created_at: number;
  status: TripStatus;
  stops: DispatchOrder[];
}

// Mirrors PaymentStatus / Invoice / CustomerBillingSummary in
// src/logistics/billing/invoice.rs. `status` is computed by the server on
// every read from paid_on / due_on / today.
export type PaymentStatus = 'PENDING' | 'PAID' | 'OVERDUE';

export interface Invoice {
  id: string;
  org_id: string;
  dispatch_id: string;
  customer_id: string;
  amount: number;
  issued_on: string;
  due_on: string;
  paid_on: string | null;
  status: PaymentStatus;
}

// Mirrors OrgRole in src/logistics/user/user.rs.
export type OrgRole = 'ADMIN' | 'DISPATCHER' | 'WAREHOUSE_STAFF';
export const ORG_ROLES: OrgRole[] = ['ADMIN', 'DISPATCHER', 'WAREHOUSE_STAFF'];
export const ROLE_LABELS: Record<OrgRole, string> = {
  ADMIN: 'Admin',
  DISPATCHER: 'Dispatcher',
  WAREHOUSE_STAFF: 'Warehouse staff',
};

export interface OrgUser {
  id: string;
  org_id: string;
  name: string;
  email: string;
  role: OrgRole;
  is_active: boolean;
}

export interface CustomerBillingSummary {
  customer_id: string;
  invoice_count: number;
  total_outstanding: number;
  overdue_count: number;
  invoices: Invoice[];
}

// Mirrors OpsReport in src/logistics/reports/mod.rs.
export interface OpsReport {
  vehicle_utilization: {
    total_vehicles: number;
    vehicles_on_active_trip: number;
    utilization_percent: number;
  };
  delivery_performance: {
    delivered_count: number;
    returned_count: number;
    avg_hours_to_deliver: number | null;
    on_time_rate_percent: number | null;
  };
  units_dispatched_recently: number;
  godown_inventory: Array<{
    godown_id: string;
    godown_name: string;
    units_on_hand: number;
    distinct_items: number;
    capacity_used_percent: number | null;
  }>;
  dispatch_volume: Array<{ date: string; count: number }>;
}

export interface ApiResponse<T> {
  success: boolean;
  message: string;
  data?: T;
}

// One fact an assistant answer was grounded in. Mirrors AssistantSource in
// src/logistics/ai/assistant.rs.
export interface AssistantSource {
  kind: string;
  source_id: string;
  excerpt: string;
}

// The org-scoped "ask your data" assistant's answer to one question.
// Mirrors AssistantAnswer in src/logistics/ai/assistant.rs.
export interface AssistantAnswer {
  answer: string;
  sources: AssistantSource[];
}
